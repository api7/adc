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

const RouteSchema = z
  .strictObject({
    uris: z.array(z.string()).min(1),
    hosts: z.array(z.string()).min(1).optional(),
    methods: z.array(z.string()).optional(),
    remote_addrs: z.array(z.string()).min(1).optional(),
    vars: z.array(z.unknown()).optional(),
    filter_func: z.string().optional(),

    plugins: Plugins.optional(),
    service_id: Metadata.id,

    timeout: Timeout.optional(),
    enable_websocket: z.boolean().optional(),
    priority: z.int().optional(),
    status: Status.optional(),
  })
  .extend(Metadata)
  .extend(ModifiedIndex);
export type Route = z.infer<typeof RouteSchema>;

const UpstreamSchema = z
  .strictObject({
    nodes: z.array(
      z.strictObject({
        host: z.string(),
        port: Port,
        weight: z.int(),
        priority: z.int().optional(),
        metadata: z.record(z.string(), z.unknown()).optional(),
      }),
    ),
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

    // Will not include health checks and service discovery configurations,
    // which are implemented by other components on the target ecosystem,
    // such as Kubernetes and Ingress Controller.
  })
  .extend(Metadata);
export type Upstream = z.infer<typeof UpstreamSchema>;

const ServiceSchema = z
  .strictObject({
    hosts: z.array(z.string()).min(1).optional(),
    upstream: UpstreamSchema.extend({
      id: Metadata.id.optional(),
      name: Metadata.name.optional(),
    }).optional(),
    plugins: Plugins.optional(),
  })
  .extend(Metadata)
  .extend(ModifiedIndex);
export type Service = z.infer<typeof ServiceSchema>;

const ConsumerSchema = z
  .strictObject({
    username: z.string(),
    desc: Metadata.desc,
    labels: Metadata.labels,

    plugins: Plugins.optional(),
  })
  .extend(ModifiedIndex);
export type Consumer = z.infer<typeof ConsumerSchema>;

const ConsumerCredentialSchema = z
  .strictObject({
    plugins: Plugins.optional(),
  })
  .extend(Metadata)
  .extend(ModifiedIndex);
export type ConsumerCredential = z.infer<typeof ConsumerCredentialSchema>;

const SSLSchema = z
  .strictObject({
    id: Metadata.id,
    desc: Metadata.desc,
    labels: Metadata.labels,

    type: z.union([z.literal('server'), z.literal('client')]).optional(),
    snis: z.array(z.string()).min(1).optional(),
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
    ssl_protocols: z.array(z.string()).optional(),
    status: Status.optional(),
  })
  .extend(ModifiedIndex);
export type SSL = z.infer<typeof SSLSchema>;

const StreamRouteSchema = z
  .strictObject({
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
  })
  .extend(Metadata)
  .extend(ModifiedIndex);
export type StreamRoute = z.infer<typeof StreamRouteSchema>;

const GlobalRuleSchema = z
  .strictObject({
    id: Metadata.id,
    plugins: Plugins.optional(),
  })
  .extend(ModifiedIndex);
export type GlobalRule = z.infer<typeof GlobalRuleSchema>;

const PluginMetadataSchema = z
  .looseObject({
    id: Metadata.id,
    // and arbitrary kv pairs
  })
  .extend(ModifiedIndex);
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

export const APISIXStandaloneKeyMap = {
  [ADCSDK.ResourceType.ROUTE]: 'routes',
  [ADCSDK.ResourceType.SERVICE]: 'services',
  [ADCSDK.ResourceType.CONSUMER]: 'consumers',
  [ADCSDK.ResourceType.SSL]: 'ssls',
  [ADCSDK.ResourceType.GLOBAL_RULE]: 'global_rules',
  [ADCSDK.ResourceType.PLUGIN_METADATA]: 'plugin_metadata',
  [ADCSDK.ResourceType.UPSTREAM]: 'upstreams',
  [ADCSDK.ResourceType.STREAM_ROUTE]: 'stream_routes',
} as const;

export const APISIXStandaloneConfVersionKeyMap = {
  [ADCSDK.ResourceType.ROUTE]: 'routes_conf_version',
  [ADCSDK.ResourceType.SERVICE]: 'services_conf_version',
  [ADCSDK.ResourceType.CONSUMER]: 'consumers_conf_version',
  [ADCSDK.ResourceType.SSL]: 'ssls_conf_version',
  [ADCSDK.ResourceType.GLOBAL_RULE]: 'global_rules_conf_version',
  [ADCSDK.ResourceType.PLUGIN_METADATA]: 'plugin_metadata_conf_version',
  [ADCSDK.ResourceType.UPSTREAM]: 'upstreams_conf_version',
  [ADCSDK.ResourceType.STREAM_ROUTE]: 'stream_routes_conf_version',
} as const;

export const ResourceLevelConfVersion = {
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.ROUTE]}`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.SERVICE]}`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.CONSUMER]}`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.SSL]}`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.GLOBAL_RULE]}`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.PLUGIN_METADATA]}`]:
    z.int().optional(),
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.UPSTREAM]}`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneConfVersionKeyMap[ADCSDK.ResourceType.STREAM_ROUTE]}`]: z
    .int()
    .optional(),
} as Record<
  `${(typeof APISIXStandaloneConfVersionKeyMap)[UsedResourceTypes]}`,
  z.ZodOptional<z.ZodInt>
>;

const Resources = {
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
};

export type ResourceFor<T extends UsedResourceTypes> =
  T extends ADCSDK.ResourceType.ROUTE
    ? Route
    : T extends ADCSDK.ResourceType.SERVICE
      ? Service
      : T extends ADCSDK.ResourceType.CONSUMER
        ? Consumer
        : T extends ADCSDK.ResourceType.SSL
          ? SSL
          : T extends ADCSDK.ResourceType.GLOBAL_RULE
            ? GlobalRule
            : T extends ADCSDK.ResourceType.PLUGIN_METADATA
              ? PluginMetadata
              : T extends ADCSDK.ResourceType.UPSTREAM
                ? Upstream
                : T extends ADCSDK.ResourceType.STREAM_ROUTE
                  ? StreamRoute
                  : never;

export const APISIXStandalone = z.strictObject(Resources);
export type APISIXStandaloneType = z.infer<typeof APISIXStandalone>;

export const APISIXStandaloneWithConfVersion = z.strictObject({
  ...Resources,
  ...ResourceLevelConfVersion,
});
export type APISIXStandaloneWithConfVersionType = z.infer<
  typeof APISIXStandaloneWithConfVersion
>;

export type ServerTokenMap = Map<string, string>;
