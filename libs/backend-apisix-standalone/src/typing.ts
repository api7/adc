import * as ADCSDK from '@api7/adc-sdk';
import { z } from 'zod';

export const ADC_UPSTREAM_SERVICE_ID_LABEL = '__ADC_UPSTREAM_SERVICE_ID';

const Metadata = {
  id: z.string(),
  name: z.string(),
  desc: z.string().optional(),
  labels: z.record(z.string(), z.string()).optional(),
};
const ModifiedIndex = {
  modifiedIndex: z.int(),
};
const Timeout = z.strictObject({
  connect: z.number().gt(0),
  send: z.number().gt(0),
  read: z.number().gt(0),
});
const Plugins = z.record(z.string(), z.record(z.string(), z.unknown()));
const Status = z.union([z.literal(0), z.literal(1)]);
const Port = z.int().min(0).max(65535);

const RouteSchema = z.strictObject({
  ...ModifiedIndex,
  ...Metadata,
  uris: z.array(z.string()).min(1),
  hosts: z.array(z.string()).min(1).optional(),
  methods: z
    .array(
      z.enum([
        'GET',
        'POST',
        'PUT',
        'DELETE',
        'PATCH',
        'HEAD',
        'OPTIONS',
        'CONNECT',
        'TRACE',
        'PURGE',
      ]),
    )
    .optional(),
  remote_addrs: z.array(z.string()).min(1).optional(),
  vars: z.array(z.unknown()).optional(),
  filter_func: z.string().optional(),

  plugins: Plugins.optional(),
  service_id: Metadata.id,

  timeout: Timeout.optional(),
  enable_websocket: z.boolean().optional(),
  priority: z.int().optional(),
  status: Status.optional(),
});
export type Route = z.infer<typeof RouteSchema>;

const upstreamHealthCheckPassiveHealthy = z.strictObject({
  http_statuses: z
    .array(z.coerce.number().int().min(200).max(599))
    .min(1)
    .default([200, 302])
    .optional(),
  successes: z.coerce.number().int().min(1).max(254).default(2).optional(),
});
const upstreamHealthCheckPassiveUnhealthy = z.strictObject({
  http_statuses: z
    .array(z.coerce.number().int().min(200).max(599))
    .min(1)
    .default([429, 404, 500, 501, 502, 503, 504, 505])
    .optional(),
  http_failures: z.coerce.number().int().min(1).max(254).default(5).optional(),
  tcp_failures: z.coerce.number().int().min(1).max(254).default(2).optional(),
  timeouts: z.coerce.number().int().min(1).max(254).default(3).optional(),
});
const upstreamHealthCheckType = z
  .union([z.literal('http'), z.literal('https'), z.literal('tcp')])
  .default('http');
const UpstreamSchema = z.strictObject({
  ...ModifiedIndex,
  ...Metadata,
  nodes: z
    .array(
      z.strictObject({
        host: z.string(),
        port: Port,
        weight: z.int(),
        priority: z.int().optional(),
        metadata: z.record(z.string(), z.unknown()).optional(),
      }),
    )
    .optional(),
  scheme: z
    .union([
      z.literal('http'),
      z.literal('https'),
      z.literal('grpc'),
      z.literal('grpcs'),
      z.literal('tcp'),
      z.literal('udp'),
      z.literal('tls'),
      z.literal('kafka'),
    ])
    .optional(),
  type: z
    .union([
      z.literal('roundrobin'),
      z.literal('chash'),
      z.literal('least_conn'),
      z.literal('ewma'),
    ])
    .optional(),
  hash_on: z.string().optional(),
  key: z.string().optional(),

  pass_host: z
    .union([z.literal('pass'), z.literal('node'), z.literal('rewrite')])
    .optional(),
  upstream_host: z.string().optional(),
  retries: z.int().optional(),
  retry_timeout: z.coerce.number().optional(),
  timeout: Timeout.optional(),
  tls: z
    .strictObject({
      client_cert_id: z.string().optional(),
      client_cert: z.string().optional(),
      client_key: z.string().optional(),
      verify: z.boolean().optional(),
    })
    .optional(),
  keepalive_pool: z
    .strictObject({
      size: z.int(),
      idle_timeout: z.int(),
      requests: z.int(),
    })
    .optional(),

  checks: z
    .strictObject({
      active: z.strictObject({
        type: upstreamHealthCheckType.optional(),
        timeout: z.coerce.number().default(1).optional(),
        concurrency: z.coerce.number().default(10).optional(),
        host: z.string().min(1).optional(),
        port: z.coerce.number().int().min(1).max(65535).optional(),
        http_path: z.string().default('/').optional(),
        https_verify_cert: z.boolean().default(true).optional(),
        http_request_headers: z.array(z.string()).min(1).optional(),
        healthy: z
          .strictObject({
            ...upstreamHealthCheckPassiveHealthy.shape,
            interval: z.coerce.number().int().min(1).default(1),
          })
          .optional(),
        unhealthy: z
          .strictObject({
            ...upstreamHealthCheckPassiveUnhealthy.shape,
            interval: z.coerce.number().int().min(1).default(1),
          })
          .optional(),
      }),
      passive: z
        .strictObject({
          type: upstreamHealthCheckType.optional(),
          healthy: upstreamHealthCheckPassiveHealthy.optional(),
          unhealthy: upstreamHealthCheckPassiveUnhealthy.optional(),
        })
        .optional(),
    })
    .optional(),
  discovery_type: z.string().optional(),
  service_name: z.string().optional(),
  discovery_args: z.record(z.string(), z.any()).optional(),
});
export type Upstream = z.infer<typeof UpstreamSchema>;

const ServiceSchema = z.strictObject({
  ...ModifiedIndex,
  ...Metadata,
  hosts: z.array(z.string()).min(1).optional(),
  upstream_id: z.string().optional(),
  plugins: Plugins.optional(),
});
export type Service = z.infer<typeof ServiceSchema>;

const ConsumerSchema = z.strictObject({
  ...ModifiedIndex,
  username: z.string(),
  desc: Metadata.desc,
  labels: Metadata.labels,

  plugins: Plugins.optional(),
});
export type Consumer = z.infer<typeof ConsumerSchema>;

const ConsumerCredentialSchema = z.strictObject({
  ...ModifiedIndex,
  ...Metadata,
  plugins: Plugins.optional(),
});
export type ConsumerCredential = z.infer<typeof ConsumerCredentialSchema>;

const SSLSchema = z
  .strictObject({
    ...ModifiedIndex,
    id: Metadata.id,
    desc: Metadata.desc,
    labels: Metadata.labels,

    type: z.union([z.literal('server'), z.literal('client')]).optional(),
    snis: z.array(z.string()).min(1),
    cert: z.string(),
    key: z.string(),
    certs: z.array(z.string()).optional(),
    keys: z.array(z.string()).optional(),
    client: z
      .strictObject({
        ca: z.string(),
        depth: z.int(),
        skip_mtls_uri_regex: z.array(z.string()).optional(),
      })
      .optional(),
    ssl_protocols: z
      .array(z.enum(['TLSv1.1', 'TLSv1.2', 'TLSv1.3']))
      .optional(),
    status: Status.optional(),
  })
  .extend(ModifiedIndex);
export type SSL = z.infer<typeof SSLSchema>;

const StreamRouteSchema = z.strictObject({
  ...ModifiedIndex,
  ...Metadata,
  remote_addr: z.string().optional(),
  server_addr: z.string().optional(),
  server_port: Port.optional(),
  sni: z.string().optional(),
  service_id: Metadata.id,
  plugins: Plugins.optional(),
  protocol: z
    .strictObject({
      name: z.string(),
      superior_id: z.string().optional(),
      conf: z.record(z.string(), z.unknown()).optional(),
      logger: z
        .array(
          z.strictObject({
            conf: z.record(z.string(), z.unknown()),
            name: z.string().optional(),
            filter: z.array(z.unknown()).optional(),
          }),
        )
        .optional(),
    })
    .optional(),
});
export type StreamRoute = z.infer<typeof StreamRouteSchema>;

const GlobalRuleSchema = z.strictObject({
  ...ModifiedIndex,
  id: Metadata.id,
  plugins: Plugins.optional(),
});
export type GlobalRule = z.infer<typeof GlobalRuleSchema>;

const PluginMetadataSchema = z.looseObject({
  ...ModifiedIndex,
  id: Metadata.id,
  // and arbitrary kv pairs
});
export type PluginMetadata = z.infer<typeof PluginMetadataSchema>;

export type UsedResourceTypes =
  | ADCSDK.ResourceType.ROUTE
  | ADCSDK.ResourceType.SERVICE
  | ADCSDK.ResourceType.CONSUMER
  | ADCSDK.ResourceType.SSL
  | ADCSDK.ResourceType.GLOBAL_RULE
  | ADCSDK.ResourceType.PLUGIN_METADATA
  | ADCSDK.ResourceType.UPSTREAM
  | ADCSDK.ResourceType.STREAM_ROUTE;

export const APISIXStandaloneKeyMap: {
  [K in UsedResourceTypes]: K extends ADCSDK.ResourceType.PLUGIN_METADATA
    ? `${K}`
    : `${K}s`;
} = {
  [ADCSDK.ResourceType.ROUTE]: 'routes',
  [ADCSDK.ResourceType.SERVICE]: 'services',
  [ADCSDK.ResourceType.CONSUMER]: 'consumers',
  [ADCSDK.ResourceType.SSL]: 'ssls',
  [ADCSDK.ResourceType.GLOBAL_RULE]: 'global_rules',
  [ADCSDK.ResourceType.PLUGIN_METADATA]: 'plugin_metadata',
  [ADCSDK.ResourceType.UPSTREAM]: 'upstreams',
  [ADCSDK.ResourceType.STREAM_ROUTE]: 'stream_routes',
} as const;

export const APISIXStandaloneConfVersionKeyMap: {
  [K in keyof typeof APISIXStandaloneKeyMap]: `${(typeof APISIXStandaloneKeyMap)[K]}_conf_version`;
} = {
  [ADCSDK.ResourceType.ROUTE]: 'routes_conf_version',
  [ADCSDK.ResourceType.SERVICE]: 'services_conf_version',
  [ADCSDK.ResourceType.CONSUMER]: 'consumers_conf_version',
  [ADCSDK.ResourceType.SSL]: 'ssls_conf_version',
  [ADCSDK.ResourceType.GLOBAL_RULE]: 'global_rules_conf_version',
  [ADCSDK.ResourceType.PLUGIN_METADATA]: 'plugin_metadata_conf_version',
  [ADCSDK.ResourceType.UPSTREAM]: 'upstreams_conf_version',
  [ADCSDK.ResourceType.STREAM_ROUTE]: 'stream_routes_conf_version',
} as const;

type ResourceFor<T extends UsedResourceTypes> =
  T extends ADCSDK.ResourceType.ROUTE
    ? typeof RouteSchema
    : T extends ADCSDK.ResourceType.SERVICE
      ? typeof ServiceSchema
      : T extends ADCSDK.ResourceType.CONSUMER
        ? z.ZodUnion<
            readonly [typeof ConsumerSchema, typeof ConsumerCredentialSchema]
          >
        : T extends ADCSDK.ResourceType.SSL
          ? typeof SSLSchema
          : T extends ADCSDK.ResourceType.GLOBAL_RULE
            ? typeof GlobalRuleSchema
            : T extends ADCSDK.ResourceType.PLUGIN_METADATA
              ? typeof PluginMetadataSchema
              : T extends ADCSDK.ResourceType.UPSTREAM
                ? typeof UpstreamSchema
                : T extends ADCSDK.ResourceType.STREAM_ROUTE
                  ? typeof StreamRouteSchema
                  : never;

// eslint-disable-next-line @typescript-eslint/no-unused-vars
const APISIXStandaloneSchema = z.strictObject({
  ...({
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.ROUTE]]: z
      .array(RouteSchema)
      .optional(),
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.SERVICE]]: z
      .array(ServiceSchema)
      .optional(),
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.CONSUMER]]: z
      .array(z.union([ConsumerSchema, ConsumerCredentialSchema]))
      .optional(),
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.SSL]]: z
      .array(SSLSchema)
      .optional(),
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.GLOBAL_RULE]]: z
      .array(GlobalRuleSchema)
      .optional(),
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.PLUGIN_METADATA]]: z
      .array(PluginMetadataSchema)
      .optional(),
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.UPSTREAM]]: z
      .array(UpstreamSchema.extend(ModifiedIndex))
      .optional(),
    [APISIXStandaloneKeyMap[ADCSDK.ResourceType.STREAM_ROUTE]]: z
      .array(StreamRouteSchema)
      .optional(),
  } as {
    [K in UsedResourceTypes as (typeof APISIXStandaloneKeyMap)[K]]: z.ZodOptional<
      z.ZodArray<ResourceFor<K>>
    >;
  }),
  ...(Object.fromEntries(
    Object.values(APISIXStandaloneConfVersionKeyMap).map((k) => [
      k,
      z.int().optional(),
    ]),
  ) as {
    [K in (typeof APISIXStandaloneConfVersionKeyMap)[keyof typeof APISIXStandaloneConfVersionKeyMap]]: z.ZodOptional<z.ZodInt>;
  }),
});

export type APISIXStandalone = z.infer<typeof APISIXStandaloneSchema>;

export type ServerTokenMap = Map<string, string>;
