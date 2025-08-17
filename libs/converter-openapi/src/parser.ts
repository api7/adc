import * as ADCSDK from '@api7/adc-sdk';
import { OpenAPIV3_1 } from '@scalar/openapi-types';

import { ExtKey } from './extension';

export const transformUpstream = (
  oasServers: Array<OpenAPIV3_1.ServerObject>,
  upstreamDefaults?: object,
) => {
  const getPort = (protocol: string): number =>
    protocol === 'http:' ? 80 : 443;
  const defaultProtocol =
    oasServers[0] && oasServers[0].url
      ? new URL(oasServers[0].url).protocol
      : 'https:';
  let defaultPathPrefix: string | undefined = undefined;
  const nodes: Array<ADCSDK.UpstreamNode> = oasServers.map((server, idx) => {
    if (server.variables)
      Object.entries(server.variables).forEach(([name, svar]) => {
        server.url = server.url!.replaceAll(`{${name}}`, `${svar.default}`);
      });
    const url = new URL(server.url!);
    if (idx === 0 && url.pathname !== '/') defaultPathPrefix = url.pathname;
    return {
      host: url.hostname,
      port: url.port ? parseInt(url.port) : getPort(url.protocol),
      weight: 100,
      ...(server as unknown as Record<string, object>)[
        ExtKey.UPSTREAM_NODE_DEFAULTS
      ],
    } as ADCSDK.UpstreamNode;
  });

  const service = {} as ADCSDK.Service;
  if (defaultPathPrefix) service.path_prefix = defaultPathPrefix;
  service.upstream = {
    scheme: defaultProtocol.slice(
      0,
      defaultProtocol.length - 1,
    ) as ADCSDK.UpstreamScheme,
    nodes,
    timeout: {
      connect: 60,
      send: 60,
      read: 60,
    },
    pass_host: 'pass',
    ...upstreamDefaults,
  };
  return service;
};

export const parseSeprateService = (
  mainService: ADCSDK.Service,
  name: string,
  context?: OpenAPIV3_1.PathItemObject | OpenAPIV3_1.OperationObject,
  pathServiceDefaults?: object, // Defaults used for operation level to merge path level
  pathUpstreamDefaults?: object, // Defaults used for operation level to merge path level
): ADCSDK.Service | undefined => {
  if (
    context &&
    (context[ExtKey.SERVICE_DEFAULTS] ||
      context[ExtKey.UPSTREAM_DEFAULTS] ||
      context.servers)
  ) {
    const separateService = {
      ...structuredClone(mainService), // Inherit all attributes from the main service
      name,
      ...(context.servers
        ? transformUpstream(context.servers, context[ExtKey.UPSTREAM_DEFAULTS])
        : {}),
    };

    // Override service and upstream defaults of path level
    return {
      ...separateService,
      upstream: {
        ...separateService.upstream,
        ...(pathUpstreamDefaults ?? {}),
        ...(context[ExtKey.UPSTREAM_DEFAULTS] ?? {}),
      },
      ...(pathServiceDefaults ?? {}),
      ...(context[ExtKey.SERVICE_DEFAULTS] ?? {}),
    } as ADCSDK.Service;
  }
  return undefined;
};
