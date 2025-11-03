import { isNil } from 'lodash';
import { type ZodRawShape, z } from 'zod';

const idSchema = z
  .string()
  .min(1)
  .max(256)
  .regex(/^[a-zA-Z0-9-_.]+$/);
const nameSchema = z
  .string()
  .min(1)
  .max(64 * 1024);
const descriptionSchema = z
  .string()
  .max(64 * 1024)
  .optional();
const labelsSchema = z.record(
  z.string(),
  z.union([z.string(), z.array(z.string())]),
);
export type Labels = z.infer<typeof labelsSchema>;
const pluginSchema = z.looseObject({});
export type Plugin = z.infer<typeof pluginSchema>;
const pluginsSchema = pluginSchema;
export type Plugins = z.infer<typeof pluginsSchema>;
const exprSchema = z.array(z.any());
export type Expr = z.infer<typeof exprSchema>;
const timeoutSchema = z.strictObject({
  connect: z.coerce.number().gt(0),
  send: z.coerce.number().gt(0),
  read: z.coerce.number().gt(0),
});
export type UpstreamHealthCheck = NonNullable<
  z.infer<ReturnType<typeof upstreamSchema>>['checks']
>;
export type UpstreamTimeout = z.infer<typeof timeoutSchema>;
const hostSchema = z.string().min(1);
const portSchema = z.coerce.number().int().min(1).max(65535);
const secretRefSchema = z.string().regex(/^\$(secret|env):\/\//);
const certificateSchema = z.union(
  [
    z
      .string()
      .min(128)
      .max(64 * 1024),
    secretRefSchema,
  ],
  { error: 'Must be a certificate string or a secret reference' },
);
const certificateKeySchema = z.union(
  [
    z
      .string()
      .min(32)
      .max(64 * 1024),
    secretRefSchema,
  ],
  { error: 'Must be a certificate private key string or a secret reference' },
);

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
  .enum(['http', 'https', 'tcp'])
  .default('http');
const upstreamNodeSchema = z.strictObject({
  host: hostSchema,
  port: portSchema,
  weight: z.coerce.number().int().min(0),
  priority: z.coerce.number().default(0).optional(),
  metadata: z.record(z.string(), z.any()).optional(),
});
export type UpstreamNode = z.infer<typeof upstreamNodeSchema>;
const upstreamBalancerSchema = z
  .enum(['roundrobin', 'chash', 'least_conn', 'ewma'])
  .default('roundrobin');
export type UpstreamBalancer = z.infer<typeof upstreamBalancerSchema>;
const upstreamSchemeSchema = z
  .enum(['grpc', 'grpcs', 'http', 'https', 'tcp', 'tls', 'udp', 'kafka'])
  .default('http');
export type UpstreamScheme = z.infer<typeof upstreamSchemeSchema>;
const upstreamPassHostSchema = z
  .enum(['pass', 'node', 'rewrite'])
  .default('pass');
export type UpstreamPassHost = z.infer<typeof upstreamPassHostSchema>;
const upstreamSchema = (extend?: ZodRawShape) =>
  z
    .strictObject({
      name: nameSchema.optional(),
      description: descriptionSchema.optional(),
      labels: labelsSchema.optional(),

      type: upstreamBalancerSchema.optional(),
      hash_on: z.string().optional(),
      key: z.string().optional(),
      checks: z
        .strictObject({
          active: z
            .strictObject({
              type: upstreamHealthCheckType.optional(),
              timeout: z.coerce.number().default(1).optional(),
              concurrency: z.coerce.number().default(10).optional(),
              host: hostSchema.optional(),
              port: portSchema.optional(),
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
            })
            .optional(),
          passive: z
            .strictObject({
              type: upstreamHealthCheckType.optional(),
              healthy: upstreamHealthCheckPassiveHealthy.optional(),
              unhealthy: upstreamHealthCheckPassiveUnhealthy.optional(),
            })
            .optional(),
        })
        .refine(
          (data) =>
            (data.active && data.passive) || (data.active && !data.passive),
          {
            message:
              'Passive health checks must be enabled at the same time as active health checks',
          },
        )
        .optional(),
      nodes: z.array(upstreamNodeSchema).optional(),
      scheme: upstreamSchemeSchema.optional(),
      retries: z.coerce.number().int().min(0).max(65535).optional(),
      retry_timeout: z.coerce.number().min(0).optional(),
      timeout: timeoutSchema.optional(),
      tls: z
        .strictObject({
          client_cert: z.string().optional(),
          client_key: z.string().optional(),
          client_cert_id: z.string().optional(),
          verify: z.boolean().optional(),
        })
        .refine(
          (data) =>
            (data.client_cert && data.client_key && !data.client_cert_id) ||
            (data.client_cert_id && !data.client_cert && !data.client_key),
          'The client_cert and client_key certificate pair or client_cert_id SSL reference ID must be set',
        )
        .optional(),
      keepalive_pool: z
        .strictObject({
          size: z.coerce.number().int().min(1).default(320),
          idle_timeout: z.coerce.number().min(0).default(60),
          requests: z.coerce.number().int().min(1).default(1000),
        })
        .optional(),
      pass_host: upstreamPassHostSchema.optional(),
      upstream_host: hostSchema.optional(),

      service_name: z.string().optional(),
      discovery_type: z.string().optional(),
      discovery_args: z.record(z.string(), z.any()).optional(),

      ...extend,
    })
    .refine(
      (val) =>
        (val.nodes && !val.discovery_type && !val.service_name) ||
        (val.discovery_type && val.service_name && !val.nodes),
      {
        error:
          'Upstream must either explicitly specify nodes or use service discovery and not both',
      },
    );
export type Upstream = z.infer<ReturnType<typeof upstreamSchema>>;

const routeSchema = z.strictObject({
  id: idSchema.optional(),
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  hosts: z.array(hostSchema).optional(),
  uris: z.array(z.string()).min(1),
  priority: z.coerce.number().int().optional(),
  timeout: timeoutSchema.optional(),
  vars: exprSchema.optional(),
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
    .nonempty()
    .optional(),
  enable_websocket: z.boolean().optional(),
  remote_addrs: z
    .array(
      z.union([z.ipv4(), z.ipv6(), z.cidrv4(), z.cidrv6()], {
        error: 'Must be IP or CIDR, accepts both IPv4 and IPv6',
      }),
    )
    .optional(),
  plugins: pluginsSchema.optional(),
  filter_func: z.string().optional(),
});
export type Route = z.infer<typeof routeSchema>;

const streamRouteSchema = z.strictObject({
  id: idSchema.optional(),
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  plugins: pluginsSchema.optional(),

  remote_addr: z.string().optional(),
  server_addr: z.string().optional(),
  server_port: portSchema.optional(),
  sni: hostSchema.optional(),
});
export type StreamRoute = z.infer<typeof streamRouteSchema>;

const serviceSchema = z
  .strictObject({
    id: idSchema.optional(),
    name: nameSchema,
    description: descriptionSchema.optional(),
    labels: labelsSchema.optional(),

    upstream: upstreamSchema().optional(),
    upstreams: z
      .array(
        upstreamSchema({
          id: idSchema.optional(),
          name: z.string(),
        }),
      )
      .optional(),
    plugins: pluginsSchema.optional(),
    path_prefix: z.string().optional(),
    strip_path_prefix: z.boolean().optional(),
    hosts: z.array(hostSchema).optional(),

    routes: z.array(routeSchema).optional(),
    stream_routes: z.array(streamRouteSchema).optional(),
  })
  .refine(
    (val) => !(Array.isArray(val.routes) && Array.isArray(val.stream_routes)),
    {
      error:
        'HTTP routes and Stream routes are mutually exclusive and should not exist in the same service',
    },
  )
  .refine((val) => !val.path_prefix || val.path_prefix.startsWith('/'), {
    error: 'Path prefix must start with "/"',
  })
  .refine((val) => !(!isNil(val.upstreams) && isNil(val.upstream)), {
    error:
      'The default upstream must be set with "upstream" when multiple upstreams are set via "upstreams"',
  });
export type Service = z.infer<typeof serviceSchema>;

const sslCertificateSchema = z.strictObject({
  certificate: certificateSchema,
  key: certificateKeySchema,
});
export type SSLCertificate = z.infer<typeof sslCertificateSchema>;
const sslSchema = z.strictObject({
  id: idSchema.optional(),
  labels: labelsSchema.optional(),

  type: z.enum(['server', 'client']).default('server').optional(),
  snis: z.array(hostSchema).min(1),
  certificates: z.array(sslCertificateSchema).refine((val) => val.length > 0, {
    error: 'SSL must contain at least one certificate',
  }),
  client: z
    .strictObject({
      ca: certificateSchema,
      depth: z.coerce.number().int().min(0).default(1),
      skip_mtls_uri_regex: z.array(z.string()).min(1).optional(),
    })
    .optional(),
  ssl_protocols: z
    .array(z.enum(['TLSv1.1', 'TLSv1.2', 'TLSv1.3']))
    .nonempty()
    .optional(),
});
export type SSL = z.infer<typeof sslSchema>;

const consumerCredentialSchema = z.strictObject({
  id: idSchema.optional(),
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  type: z
    .string()
    .refine(
      (type) =>
        ['key-auth', 'basic-auth', 'jwt-auth', 'hmac-auth'].includes(type),
      {
        message:
          'Consumer credential only supports "key-auth", "basic-auth", "jwt-auth" and "hmac-auth" types',
      },
    ),
  config: pluginSchema,
});
export type ConsumerCredential = z.infer<typeof consumerCredentialSchema>;
const consumerSchema = z.strictObject({
  username: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  plugins: pluginsSchema.optional(),
  credentials: z.array(consumerCredentialSchema).optional(),
});
export type Consumer = z.infer<typeof consumerSchema>;

const consumerGroupSchema = z.strictObject({
  id: idSchema.optional(),
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  plugins: pluginsSchema,

  consumers: z.array(consumerSchema).optional(),
});
export type ConsumerGroup = z.infer<typeof consumerGroupSchema>;

const globalRuleSchema = pluginsSchema;
export type GlobalRule = z.infer<typeof globalRuleSchema>;

const pluginMetadataSchema = pluginsSchema;
export type PluginMetadata = z.infer<typeof pluginMetadataSchema>;

export const ConfigurationSchema = z.strictObject({
  services: z.array(serviceSchema).optional(),
  ssls: z.array(sslSchema).optional(),
  consumers: z.array(consumerSchema).optional(),
  consumer_groups: z.array(consumerGroupSchema).optional(),
  global_rules: globalRuleSchema.optional(),
  plugin_metadata: pluginMetadataSchema.optional(),
  routes: z.array(routeSchema).optional(),
  stream_routes: z.array(streamRouteSchema).optional(),
});
export type Configuration = z.infer<typeof ConfigurationSchema>;

export const InternalConfigurationSchema = z.strictObject({
  services: z.array(serviceSchema).optional(),
  ssls: z.array(sslSchema).optional(),
  consumers: z.array(consumerSchema).optional(),

  // object format resources
  global_rules: z.record(z.string(), globalRuleSchema).optional(),
  plugin_metadata: z.record(z.string(), pluginMetadataSchema).optional(),

  // internal use only
  routes: z.array(routeSchema).optional(),
  stream_routes: z.array(streamRouteSchema).optional(),
  consumer_credentials: z.array(consumerCredentialSchema).optional(),
  upstreams: z.array(upstreamSchema({ id: idSchema.optional() })).optional(),
});
export type InternalConfiguration = z.infer<typeof InternalConfigurationSchema>;
