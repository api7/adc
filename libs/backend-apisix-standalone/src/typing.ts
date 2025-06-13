import * as ADCSDK from '@api7/adc-sdk';
import { z } from 'zod/v4';

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

export const Route = z
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

const Upstream = z
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

const Service = z
  .strictObject({
    hosts: z.array(z.string()).min(1).optional(),
    upstream: Upstream.extend({
      id: Metadata.id.optional(),
      name: Metadata.name.optional(),
    }).optional(),
    plugins: Plugins.optional(),
  })
  .extend(Metadata)
  .extend(ModifiedIndex);

const Consumer = z
  .strictObject({
    username: z.string(),
    desc: Metadata.desc,
    labels: Metadata.labels,

    plugins: Plugins.optional(),
  })
  .extend(ModifiedIndex);

const ConsumerCredential = z
  .strictObject({
    plugins: Plugins.optional(),
  })
  .extend(Metadata)
  .extend(ModifiedIndex);

const SSL = z
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

const StreamRoute = z
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

const GlobalRule = z
  .strictObject({
    id: Metadata.id,
    plugins: Plugins.optional(),
  })
  .extend(ModifiedIndex);

const PluginMetadata = z
  .looseObject({
    id: Metadata.id,
    // and arbitrary kv pairs
  })
  .extend(ModifiedIndex);

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

export const ResourceLevelConfVersion = {
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.ROUTE]}_conf_version`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.SERVICE]}_conf_version`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.CONSUMER]}_conf_version`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.SSL]}_conf_version`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.GLOBAL_RULE]}_conf_version`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.PLUGIN_METADATA]}_conf_version`]:
    z.int().optional(),
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.UPSTREAM]}_conf_version`]: z
    .int()
    .optional(),
  [`${APISIXStandaloneKeyMap[ADCSDK.ResourceType.STREAM_ROUTE]}_conf_version`]:
    z.int().optional(),

  protos_conf_version: z.int().optional(),
  plugin_configs_conf_version: z.int().optional(),
  consumer_groups_conf_version: z.int().optional(),
  secrets_conf_version: z.int().optional(),
};

const Resources = {
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.ROUTE]]: z
    .array(Route)
    .optional(),
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.SERVICE]]: z
    .array(Service)
    .optional(),
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.CONSUMER]]: z
    .array(z.union([Consumer, ConsumerCredential]))
    .optional(),
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.SSL]]: z.array(SSL).optional(),
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.GLOBAL_RULE]]: z
    .array(GlobalRule)
    .optional(),
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.PLUGIN_METADATA]]: z
    .array(PluginMetadata)
    .optional(),
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.UPSTREAM]]: z
    .array(Upstream.extend(ModifiedIndex))
    .optional(),
  [APISIXStandaloneKeyMap[ADCSDK.ResourceType.STREAM_ROUTE]]: z
    .array(StreamRoute)
    .optional(),
};

export const APISIXStandalone = z.strictObject(Resources);
export type APISIXStandaloneType = z.infer<typeof APISIXStandalone>;

export const APISIXStandaloneWithConfVersion = z.strictObject({
  ...Resources,
  ...ResourceLevelConfVersion,
});
export type APISIXStandaloneWithConfVersionType = z.infer<
  typeof APISIXStandaloneWithConfVersion
>;
