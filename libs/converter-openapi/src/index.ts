import * as ADCSDK from '@api7/adc-sdk';
import { dereference, upgrade } from '@scalar/openapi-parser';
import { OpenAPIV3_1 } from '@scalar/openapi-types';
import { isEmpty, unset } from 'lodash-es';
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

type UnknownRecord = Record<string, unknown>;

const copiedExtensionKeys = [
  ExtKey.NAME,
  ExtKey.LABELS,
  ExtKey.PLUGINS,
  ExtKey.SERVICE_DEFAULTS,
  ExtKey.UPSTREAM_DEFAULTS,
  ExtKey.UPSTREAM_NODE_DEFAULTS,
  ExtKey.ROUTE_DEFAULTS,
];

const isRecord = (value: unknown): value is UnknownRecord =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const copyKey = (source: UnknownRecord, target: UnknownRecord, key: string) => {
  if (source[key] !== undefined) target[key] = source[key];
};

const copyExtensionKeys = (source: UnknownRecord, target: UnknownRecord) => {
  copiedExtensionKeys.forEach((key) => copyKey(source, target, key));
  Object.entries(source)
    .filter(([key]) => key.startsWith(ExtKey.PLUGIN_PREFIX))
    .forEach(([key, value]) => {
      target[key] = value;
    });
};

const pruneOperation = (operation: unknown): unknown => {
  if (!isRecord(operation)) return operation;

  const result: UnknownRecord = {};
  ['$ref', 'operationId', 'summary', 'description', 'servers'].forEach((key) =>
    copyKey(operation, result, key),
  );
  copyExtensionKeys(operation, result);
  return result;
};

const prunePathItem = (pathItem: unknown): unknown => {
  if (!isRecord(pathItem)) return pathItem;

  const result: UnknownRecord = {};
  ['$ref', 'servers'].forEach((key) => copyKey(pathItem, result, key));
  copyExtensionKeys(pathItem, result);
  httpMethods.forEach((method) => {
    if (pathItem[method] !== undefined)
      result[method] = pruneOperation(pathItem[method]);
  });
  return result;
};

const prunePathItems = (pathItems: UnknownRecord): UnknownRecord =>
  Object.fromEntries(
    Object.entries(pathItems).map(([path, pathItem]) => [
      path,
      prunePathItem(pathItem),
    ]),
  );

const createConversionDocument = (
  specification: unknown,
): OpenAPIV3_1.Document => {
  const source = specification as unknown as UnknownRecord;
  const result: UnknownRecord = {};

  ['openapi', 'info', 'servers'].forEach((key) => copyKey(source, result, key));
  copyExtensionKeys(source, result);

  if (isRecord(source.paths)) result.paths = prunePathItems(source.paths);

  const components = source.components;
  if (isRecord(components) && isRecord(components.pathItems)) {
    result.components = {
      pathItems: prunePathItems(components.pathItems),
    };
  }

  return result as unknown as OpenAPIV3_1.Document;
};

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
              service.routes = service.routes!.map((route: ADCSDK.Route) => {
                route.uris = route.uris.map(
                  (uri: string) => `${service.path_prefix}${uri}`,
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
    return of(upgrade(content).specification).pipe(
      map((res) => {
        if (!res) throw new Error('No schema found in OpenAPI document');
        return createConversionDocument(res);
      }),
      map((specification) => dereference(specification)),
      map((res) => {
        if (res.errors?.length)
          throw new Error(
            `Failed to dereference OpenAPI document: ${res.errors
              .map((error) => error.message)
              .join(', ')}`,
          );
        if (!res.schema) throw new Error('No schema found in OpenAPI document');
        return res.schema as OpenAPIV3_1.Document;
      }),
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
