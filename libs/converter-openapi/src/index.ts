import * as ADCSDK from '@api7/adc-sdk';
import { Listr } from 'listr2';
import { isEmpty } from 'lodash';
import { OpenAPIV3 } from 'openapi-types';
import slugify from 'slugify';
import { z } from 'zod';

import { ExtKey, parseExtPlugins } from './extension';
import { parseSeprateService, parseUpstream } from './parser';
import { schema } from './schema';

const pathVariableRegex = /{[^}]+}/g;

export class OpenAPIConverter implements ADCSDK.Converter {
  public toADC(
    oas: OpenAPIV3.Document,
  ): Listr<{ local: ADCSDK.Configuration }> {
    return new Listr<{ local: ADCSDK.Configuration }>([
      {
        title: 'Validate OpenAPI document',
        task: (ctx, task) => {
          const result = schema.safeParse(oas);

          if (!result.success) {
            const err = `Validate OpenAPI document\nThe following errors were found in OpenAPI document:\n${z.prettifyError(result.error)}`;
            const error = new Error(err);
            error.stack = '';
            throw error;
          }
        },
      },
      {
        title: 'Generate main service',
        task: (ctx) => {
          ctx.local.services = [
            {
              name: oas[ExtKey.NAME]
                ? oas[ExtKey.NAME]
                : oas.info.title
                  ? oas.info.title
                  : 'Untitled service',
              description: oas.info.description,
              labels: oas[ExtKey.LABELS],
              plugins: parseExtPlugins(oas),
              routes: [],
            },
          ];

          parseUpstream(
            ctx.local.services[0],
            oas.servers,
            oas[ExtKey.UPSTREAM_DEFAULTS],
          );

          ctx.local.services[0] = {
            ...ctx.local.services[0],
            ...(oas[ExtKey.SERVICE_DEFAULTS] ?? {}),
          };
        },
      },
      {
        title: 'Generate routes',
        task: (ctx, task) => {
          const mainService = ctx.local.services[0];
          const services: Array<ADCSDK.Service> = [];
          Object.entries(oas.paths).forEach(([path, operations]) => {
            const httpMethods = Object.values(OpenAPIV3.HttpMethods);

            const separateService = parseSeprateService(
              mainService,
              operations,
              [mainService.name, path].map((item) => slugify(item)).join('_'),
            );
            const pathPlugins = parseExtPlugins(operations);
            const routes = Object.entries(operations)
              .filter(([key]) =>
                httpMethods.includes(key as OpenAPIV3.HttpMethods),
              )
              .map(
                ([method, operation]: [
                  OpenAPIV3.HttpMethods,
                  OpenAPIV3.OperationObject,
                ]) => {
                  const separateService = parseSeprateService(
                    mainService,
                    operation,
                    [oas.info.title, path, method]
                      .map((item) => slugify(item))
                      .join('_'),
                    operations[ExtKey.SERVICE_DEFAULTS],
                    {
                      ...oas[ExtKey.UPSTREAM_DEFAULTS],
                      ...operations[ExtKey.UPSTREAM_DEFAULTS],
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
                        : [oas.info.title, path, method]
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
                    ...structuredClone(operations[ExtKey.ROUTE_DEFAULTS]),
                    ...structuredClone(operation[ExtKey.ROUTE_DEFAULTS]),
                  } as ADCSDK.Route);

                  if (separateService) {
                    separateService.routes = [route];
                    services.push(separateService);
                    task.output = this.buildDebugOutput([
                      `${method.toUpperCase()} "${path}" contains the service or upstream defaults, so it will be included to the separate service`,
                    ]);
                  } else {
                    return route;
                  }
                },
              )
              .filter((item) => !!item);
            if (separateService) {
              if (routes?.length <= 0) return;
              separateService.routes = routes;
              services.push(separateService);
              task.output = this.buildDebugOutput([
                `Path "${path}" contains the service or upstream defaults, so it will be included to the separate service`,
              ]);
            } else {
              mainService.routes.push(...routes);
            }
          });
          ctx.local.services = ctx.local.services
            .concat(...services)
            .filter((service) => service?.routes?.length > 0)
            .map((service) => ADCSDK.utils.recursiveOmitUndefined(service));
        },
      },
    ]);
  }

  private buildDebugOutput(messages: Array<string>) {
    return JSON.stringify({
      type: 'DEBUG',
      messages,
    });
  }
}
