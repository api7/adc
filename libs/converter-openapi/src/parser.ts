import * as ADCSDK from '@api7/adc-sdk';
import { OpenAPIV3 } from 'openapi-types';

import { ExtKey, parseExtPlugins } from './extension';

export const parseUpstream = (
  service: ADCSDK.Service,
  oasServers: Array<OpenAPIV3.ServerObject>,
  upstreamDefaults?: object,
) => {
  const getPort = (protocol: string): number =>
    protocol === 'http:' ? 80 : 443;
  const defaultProtocol = oasServers[0]
    ? new URL(oasServers[0].url).protocol
    : 'https:';
  let defaultPathPrefix: string;
  const nodes: Array<ADCSDK.UpstreamNode> = oasServers.map((server, idx) => {
    if (server.variables)
      Object.entries(server.variables).forEach(([name, svar]) => {
        server.url = server.url.replaceAll(`{${name}}`, svar.default);
      });

    const url = new URL(server.url);
    if (idx === 0) defaultPathPrefix = url.pathname;

    return {
      host: url.hostname,
      port: url.port ? parseInt(url.port) : getPort(url.protocol),
      weight: 100,
      ...server[ExtKey.UPSTREAM_NODE_DEFAULTS],
    } as ADCSDK.UpstreamNode;
  });

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
};

export const parseSeprateService = (
  mainService: ADCSDK.Service,
  context: OpenAPIV3.PathItemObject | OpenAPIV3.OperationObject,
  name: string,
  pathServiceDefaults?: object, // Defaults used for operation level to merge path level
  pathUpstreamDefaults?: object, // Defaults used for operation level to merge path level
): ADCSDK.Service | undefined => {
  let separateService: ADCSDK.Service = undefined;
  if (
    context[ExtKey.SERVICE_DEFAULTS] ||
    context[ExtKey.UPSTREAM_DEFAULTS] ||
    context.servers
  ) {
    separateService = {
      ...structuredClone(mainService), // Inherit all attributes from the main service
      name,
    };

    // Override upstream of path level
    if (context.servers)
      parseUpstream(
        separateService,
        context.servers,
        context[ExtKey.UPSTREAM_DEFAULTS],
      );

    // Override service and upstream defaults of path level
    separateService = {
      ...separateService,
      upstream: {
        ...separateService.upstream,
        ...(pathUpstreamDefaults ?? {}),
        ...(context[ExtKey.UPSTREAM_DEFAULTS] ?? {}),
      },
      ...(pathServiceDefaults ?? {}),
      ...(context[ExtKey.SERVICE_DEFAULTS] ?? {}),
    };
  }
  return separateService;
};
