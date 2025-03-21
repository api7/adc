import { z } from 'zod';

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
const pluginSchema = z.record(z.string(), z.any());
const pluginsSchema = z.record(z.string(), pluginSchema);
const exprSchema = z.array(z.any());
const timeoutSchema = z.object({
  connect: z.coerce.number().gt(0),
  send: z.coerce.number().gt(0),
  read: z.coerce.number().gt(0),
});
const hostSchema = z.string().min(1);
const portSchema = z.coerce.number().int().min(1).max(65535);
const secretRefSchema = z.string().regex(/^\$(secret|env):\/\//);
const certificateSchema = z.union([
  z
    .string()
    .min(128)
    .max(64 * 1024),
  secretRefSchema,
]);
const certificateKeySchema = z.union([
  z
    .string()
    .min(32)
    .max(64 * 1024),
  secretRefSchema,
]);

const upstreamHealthCheckPassiveHealthy = z
  .object({
    http_statuses: z
      .array(z.coerce.number().int().min(200).max(599))
      .min(1)
      .default([200, 302])
      .optional(),
    successes: z.coerce.number().int().min(1).max(254).default(2).optional(),
  })
  .strict();
const upstreamHealthCheckPassiveUnhealthy = z
  .object({
    http_statuses: z
      .array(z.coerce.number().int().min(200).max(599))
      .min(1)
      .default([429, 404, 500, 501, 502, 503, 504, 505])
      .optional(),
    http_failures: z.coerce
      .number()
      .int()
      .min(1)
      .max(254)
      .default(5)
      .optional(),
    tcp_failures: z.coerce.number().int().min(1).max(254).default(2).optional(),
    timeouts: z.coerce.number().int().min(1).max(254).default(3).optional(),
  })
  .strict();
const upstreamHealthCheckType = z
  .enum(['http', 'https', 'tcp'])
  .default('http');
const upstreamSchema = z
  .object({
    name: nameSchema.optional(),
    description: descriptionSchema.optional(),
    labels: labelsSchema.optional(),

    type: z
      .enum(['roundrobin', 'chash', 'least_conn', 'ewma'])
      .default('roundrobin')
      .optional(),
    hash_on: z.string().optional(),
    key: z.string().optional(),
    checks: z
      .object({
        active: z.object({
          type: upstreamHealthCheckType.optional(),
          timeout: z.coerce.number().default(1).optional(),
          concurrency: z.coerce.number().default(10).optional(),
          host: hostSchema.optional(),
          port: portSchema.optional(),
          http_path: z.string().default('/').optional(),
          https_verify_cert: z.boolean().default(true).optional(),
          http_request_headers: z.array(z.string()).min(1).optional(),
          healthy: z
            .object({
              interval: z.coerce.number().int().min(1).default(1),
            })
            .merge(upstreamHealthCheckPassiveHealthy)
            .strict()
            .optional(),
          unhealthy: z
            .object({
              interval: z.coerce.number().int().min(1).default(1),
            })
            .merge(upstreamHealthCheckPassiveUnhealthy)
            .strict()
            .optional(),
        }),
        passive: z
          .object({
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
    nodes: z
      .array(
        z.object({
          host: hostSchema,
          port: portSchema.optional(),
          weight: z.coerce.number().int().min(0),
          priority: z.coerce.number().default(0).optional(),
          metadata: z.record(z.string(), z.any()).optional(),
        }),
      )
      .optional(),
    scheme: z
      .enum(['grpc', 'grpcs', 'http', 'https', 'tcp', 'tls', 'udp', 'kafka'])
      .default('http')
      .optional(),
    retries: z.coerce.number().int().min(0).max(65535).optional(),
    retry_timeout: z.coerce.number().min(0).optional(),
    timeout: timeoutSchema.optional(),
    tls: z
      .object({
        client_cert: z.string().optional(),
        client_key: z.string().optional(),
        client_cert_id: z.string().optional(),
        verify: z.boolean().optional(),
      })
      .strict()
      .refine(
        (data) =>
          (data.client_cert && data.client_key && !data.client_cert_id) ||
          (data.client_cert_id && !data.client_cert && !data.client_key),
        'The client_cert and client_key certificate pair or client_cert_id SSL reference ID must be set',
      )
      .optional(),
    keepalive_pool: z
      .object({
        size: z.coerce.number().int().min(1).default(320),
        idle_timeout: z.coerce.number().min(0).default(60),
        requests: z.coerce.number().int().min(1).default(1000),
      })
      .optional(),
    pass_host: z.enum(['pass', 'node', 'rewrite']).default('pass').optional(),
    upstream_host: hostSchema.optional(),

    service_name: z.string().optional(),
    discovery_type: z.string().optional(),
    discovery_args: z.record(z.string(), z.any()).optional(),
  })
  .strict()
  .refine(
    (val) =>
      (val.nodes && !val.discovery_type && !val.service_name) ||
      (val.discovery_type && val.service_name && !val.nodes),
    {
      message:
        'Upstream must either explicitly specify nodes or use service discovery and not both',
    },
  );

const routeSchema = z
  .object({
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
    remote_addrs: z.array(z.string().ip()).optional(),
    plugins: pluginsSchema.optional(),
    filter_func: z.string().optional(),
  })
  .strict();

const streamRouteSchema = z
  .object({
    id: idSchema.optional(),
    name: nameSchema,
    description: descriptionSchema.optional(),
    labels: labelsSchema.optional(),

    plugins: pluginsSchema.optional(),

    remote_addr: z.string().optional(),
    server_addr: z.string().optional(),
    server_port: portSchema.optional(),
    sni: hostSchema.optional(),
  })
  .strict();

const serviceSchema = z
  .object({
    id: idSchema.optional(),
    name: nameSchema,
    description: descriptionSchema.optional(),
    labels: labelsSchema.optional(),

    upstream: upstreamSchema.optional(),
    plugins: pluginsSchema.optional(),
    path_prefix: z.string().optional(),
    strip_path_prefix: z.boolean().optional(),
    hosts: z.array(hostSchema).optional(),

    routes: z.array(routeSchema).optional(),
    stream_routes: z.array(streamRouteSchema).optional(),
  })
  .strict()
  .refine(
    (val) => !(Array.isArray(val.routes) && Array.isArray(val.stream_routes)),
    {
      message:
        'HTTP routes and Stream routes are mutually exclusive and should not exist in the same service',
    },
  )
  .refine(
    (val) => {
      if (!val.path_prefix) return true;
      return val.path_prefix.startsWith('/');
    },
    {
      message: 'Path prefix must start with "/"',
    },
  );

const sslSchema = z
  .object({
    id: idSchema.optional(),
    labels: labelsSchema.optional(),

    type: z.enum(['server', 'client']).default('server').optional(),
    snis: z.array(hostSchema).min(1),
    certificates: z
      .array(
        z
          .object({
            certificate: certificateSchema,
            key: certificateKeySchema,
          })
          .strict(),
      )
      .refine((val) => val.length > 0, {
        message: 'SSL must contain at least one certificate',
      }),
    client: z
      .object({
        ca: certificateSchema,
        depth: z.coerce.number().int().min(0).default(1).optional(),
        skip_mtls_uri_regex: z.array(z.string()).min(1).optional(),
      })
      .strict()
      .optional(),
    ssl_protocols: z
      .array(z.enum(['TLSv1.1', 'TLSv1.2', 'TLSv1.3']))
      .nonempty()
      .optional(),
  })
  .strict();

const consumerCredentialSchema = z
  .object({
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
  })
  .strict();
const consumerSchema = z
  .object({
    username: nameSchema,
    description: descriptionSchema.optional(),
    labels: labelsSchema.optional(),

    plugins: pluginsSchema.optional(),
    credentials: z.array(consumerCredentialSchema).optional(),
  })
  .strict();

const consumerGroupSchema = z
  .object({
    id: idSchema.optional(),
    name: nameSchema,
    description: descriptionSchema.optional(),
    labels: labelsSchema.optional(),

    plugins: pluginsSchema,

    consumers: z.array(consumerSchema).optional(),
  })
  .strict();

export const ConfigurationSchema = z
  .object({
    services: z.array(serviceSchema).optional(),
    ssls: z.array(sslSchema).optional(),
    consumers: z.array(consumerSchema).optional(),
    consumer_groups: z.array(consumerGroupSchema).optional(),
    global_rules: pluginsSchema.optional(),
    plugin_metadata: pluginsSchema.optional(),
  })
  .strict();
