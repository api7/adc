import * as ADCSDK from '@api7/adc-sdk';
import { dereference, upgrade } from '@scalar/openapi-parser';
import { OpenAPIV3_1 } from '@scalar/openapi-types';
import { isEmpty, unset } from 'lodash';
import { Observable, from, map, of, switchMap, tap } from 'rxjs';
import slugify from 'slugify';
import { z } from 'zod';

import { ExtKey, parseExtPlugins } from './extension';
import { parseSeprateService, transformUpstream } from './parser';
import { schema } from './schema';

const pathVariableRegex = /{[^}]+}/g;
const httpMethods: Array<OpenAPIV3_1.HttpMethods> = [
  'get',
  'put',
  'post',
  'delete',
  'options',
  'head',
  'patch',
  'trace',
];

export class OpenAPIConverter implements ADCSDK.Converter {
  public toADC(content: string): Observable<ADCSDK.Configuration> {
    return from(this.parseOAS(content)).pipe(
      switchMap((oas) => {
        return of([] as Array<ADCSDK.Service>).pipe(
          // Generate main service
          tap((services) => {
            const { path_prefix, upstream } = transformUpstream(
              // eslint-disable-next-line @typescript-eslint/no-non-null-assertion -- checked by schema
              oas.servers!,
              oas[ExtKey.UPSTREAM_DEFAULTS],
            );
            services.push({
              name: oas[ExtKey.NAME]
                ? oas[ExtKey.NAME]
                : oas.info?.title
                  ? oas.info.title
                  : 'Untitled service',
              description: oas.info?.description,
              labels: oas[ExtKey.LABELS],
              plugins: parseExtPlugins(oas),
              routes: [],
              path_prefix,
              upstream,
              ...(oas[ExtKey.SERVICE_DEFAULTS] ?? {}),
            });
          }),
          // Check those paths with special upstream configurations and generate split services
          tap((services) => {
            const mainService = services[0];
            Object.entries(oas.paths!).forEach(([path, operations]) => {
              const separateService = parseSeprateService(
                mainService,
                [mainService.name, path].map((item) => slugify(item)).join('_'),
                operations,
              );
              const pathPlugins = operations ? parseExtPlugins(operations) : {};
              const routes = Object.entries<OpenAPIV3_1.OperationObject>(
                operations ?? {},
              )
                .filter(([method]) =>
                  httpMethods.includes(method as OpenAPIV3_1.HttpMethods),
                )
                .map(([method, operation]) => {
                  const separateService = parseSeprateService(
                    mainService,
                    [oas.info!.title!, path, method]
                      .map((item) => slugify(item))
                      .join('_'),
                    operation,
                    operations![ExtKey.SERVICE_DEFAULTS],
                    {
                      ...oas[ExtKey.UPSTREAM_DEFAULTS],
                      ...operations![ExtKey.UPSTREAM_DEFAULTS],
                    },
                  );

                  const plugins = {
                    ...pathPlugins,
                    ...parseExtPlugins(operation),
                  };
                  const route = ADCSDK.utils.recursiveOmitUndefined({
                    name: operation[ExtKey.NAME]
                      ? operation[ExtKey.NAME]
                      : operation.operationId
                        ? operation.operationId
                        : [oas.info!.title!, path, method]
                            .map((item) => slugify(item))
                            .join('_'),
                    description: operation.summary ?? operation.description,
                    labels: operation[ExtKey.LABELS],
                    methods: [method.toUpperCase()],
                    uris: [
                      path.replaceAll(pathVariableRegex, (match) => {
                        return `:${match.slice(1, match.length - 1)}`;
                      }),
                    ],
                    plugins: !isEmpty(plugins) ? plugins : undefined,
                    ...structuredClone(oas[ExtKey.ROUTE_DEFAULTS]),
                    ...structuredClone(operations![ExtKey.ROUTE_DEFAULTS]),
                    ...structuredClone(operation[ExtKey.ROUTE_DEFAULTS]),
                  } as ADCSDK.Route);

                  if (separateService) {
                    separateService.routes = [route];
                    services.push(separateService);
                    //TODO use event subscription
                    console.log(
                      `${method.toUpperCase()} "${path}" contains the service or upstream defaults, so it will be included to the separate service`,
                    );
                    return undefined;
                  } else {
                    return route;
                  }
                })
                .filter((item) => !!item);
              if (separateService) {
                if (routes?.length <= 0) return;
                separateService.routes = routes;
                services.unshift(separateService);
                //TODO use event subscription
                console.log(
                  `Path "${path}" contains the service or upstream defaults, so it will be included to the separate service`,
                );
              } else {
                mainService.routes!.push(...routes);
              }
            });
          }),
          // Always inline path prefix to support APISIX
          tap((services) =>
            services.map((service) => {
              if (!service.path_prefix) return service;
              service.routes = service.routes!.map((route) => {
                route.uris = route.uris.map(
                  (uri) => `${service.path_prefix}${uri}`,
                );
                return route;
              });
              unset(service, 'path_prefix');
              return service;
            }),
          ),
          map((services) =>
            services
              .filter((service) => service.routes && service.routes?.length > 0)
              .map((service) => ADCSDK.utils.recursiveOmitUndefined(service)),
          ),
          map((services) => ({ services }) as ADCSDK.Configuration),
        );
      }),
    );
  }

  private parseOAS(content: string): Observable<OpenAPIV3_1.Document> {
    return of(dereference(content)).pipe(
      map((res) => {
        if (res.errors?.length)
          throw new Error(
            `Failed to dereference OpenAPI document: ${res.errors
              .map((error) => error.message)
              .join(', ')}`,
          );
        if (!res.schema) throw new Error('No schema found in OpenAPI document');
        return res.schema;
      }),
      map((schema) => upgrade(schema).specification),
      tap((specification) => {
        const result = schema.safeParse(specification);

        if (!result.success) {
          const err = `Validate OpenAPI document\nThe following errors were found in OpenAPI document:\n${z.prettifyError(result.error)}`;
          const error = new Error(err);
          error.stack = '';
          throw error;
        }
      }),
    );
  }
}
