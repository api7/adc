import { z } from 'zod';

const nameSchema = z.string().min(1).max(100);
const descriptionSchema = z.string().max(256).optional();
const labelsSchema = z.record(
  z.string(),
  z.union([z.string(), z.array(z.string())]),
);
const pluginsSchema = z.record(z.string(), z.record(z.string(), z.any()));
const exprSchema = z.array(
  z.union([z.string(), z.array(z.lazy(() => exprSchema))]),
);
const timeoutSchema = z.object({
  connect: z.number(),
  send: z.number(),
  read: z.number(),
});
const portSchema = z.number().int().min(1).max(65535);
const certificateSchema = z
  .string()
  .min(128)
  .max(64 * 1024);

const upstreamHealthCheckPassiveHealthy = z
  .object({
    http_statuses: z
      .array(z.number().int().min(200).max(599))
      .min(1)
      .default([200, 302])
      .optional(),
    successes: z.number().int().min(1).max(254).default(2).optional(),
  })
  .strict();
const upstreamHealthCheckPassiveUnhealthy = z
  .object({
    http_statuses: z
      .array(z.number().int().min(200).max(599))
      .min(1)
      .default([200, 302])
      .optional(),
    http_failures: z.number().int().min(1).max(254).default(5).optional(),
    tcp_failures: z.number().int().min(1).max(254).default(2).optional(),
    timeouts: z.number().int().min(1).max(254).default(3).optional(),
  })
  .strict();
const upstreamHealthCheckType = z
  .enum(['http', 'https', 'tcp'])
  .default('http');
const upstreamSchema = z.object({
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  type: z.enum(['roundrobin', 'chash', 'least_conn', 'ewma']).optional(),
  hash_on: z.string().optional(),
  key: z.string().optional(),
  checks: z
    .object({
      active: z
        .object({
          type: upstreamHealthCheckType.optional(),
          timeout: z.number().default(1).optional(),
          concurrency: z.number().default(10).optional(),
          host: z.string(), //TODO
          port: portSchema,
          http_path: z.string().default('/').optional(),
          https_verify_cert: z.boolean().default(true).optional(),
          http_request_headers: z.array(z.string()).min(1).optional(),
          healthy: z
            .object({
              interval: z.number().int().min(1).default(1),
            })
            .merge(upstreamHealthCheckPassiveHealthy)
            .strict()
            .optional(),
          unhealthy: z
            .object({
              interval: z.number().int().min(1).default(1),
            })
            .merge(upstreamHealthCheckPassiveUnhealthy)
            .strict()
            .optional(),
        })
        .optional(),
      passive: z
        .object({
          type: upstreamHealthCheckType.optional(),
          healthy: upstreamHealthCheckPassiveHealthy.optional(),
          unhealthy: upstreamHealthCheckPassiveUnhealthy.optional(),
        })
        .optional(),
    })
    .refine(
      (data) => (data.active && data.passive) || (data.active && !data.passive),
    )
    .optional(),
  nodes: z.array(
    z.object({
      host: z.string(),
      port: portSchema.optional(),
      weight: z.number().int().min(0),
      priority: z.number().default(0).optional(),
      metadata: z.record(z.string(), z.any()).optional(),
    }),
  ),
  scheme: z
    .enum(['grpc', 'grpcs', 'http', 'https', 'tcp', 'tls', 'udp', 'kafka'])
    .default('http')
    .optional(),
  retries: z.number().int().min(0).optional(),
  retry_timeout: z.number().min(0).optional(),
  timeout: timeoutSchema.optional(),
  tls: z
    .object({
      cert: z.string(),
      key: z.string(),
      client_cert_id: z.string(),
      verify: z.boolean(),
    })
    .refine(
      (data) =>
        (data.cert && data.key && !data.client_cert_id) ||
        (data.client_cert_id && !data.cert && !data.key),
    )
    .optional(),
  keepalive_pool: z
    .object({
      size: z.number().int().min(1).default(320),
      idle_timeout: z.number().min(0).default(60),
      requests: z.number().int().min(1).default(1000),
    })
    .optional(),
  pass_host: z.enum(['pass', 'node', 'rewrite']).default('pass').optional(),
  upstream_host: z.string().optional(),

  service_name: z.string().optional(),
  discovery_type: z.string().optional(),
  discovery_args: z.record(z.string(), z.any()).optional(),
});

const refineUpstream = <T extends z.ZodRawShape>(obj: z.ZodObject<T>) => {
  return obj.refine(
    (data) =>
      (data.nodes && !data.discovery_type && !data.service_name) ||
      (data.discovery_type && data.service_name && !data.nodes),
  );
};

const routeSchema = z.object({
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  hosts: z.array(z.string()).optional(),
  uris: z.array(z.string()).min(1),
  priority: z.number().int().optional(),
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
    .optional(),
  enable_websocket: z.boolean().optional(),
  remote_addrs: z.array(z.string().ip()).optional(),
  plugins: pluginsSchema.optional(),

  plugin_config_id: z.string().optional(),
  filter_func: z.string().optional(),
});

const streamRouteSchema = z.object({
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  remote_addr: z.string().optional(),
  server_addr: z.string().optional(),
  server_port: portSchema.optional(),
  sni: z.string().optional(),
});

const serviceSchema = z.object({
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  upstream: refineUpstream(
    upstreamSchema.extend({ name: nameSchema.optional() }),
  ).optional(),
  plugins: pluginsSchema.optional(),

  routes: z.array(routeSchema).optional(),
  stream_routes: z.array(streamRouteSchema).optional(),
});

const sslSchema = z.object({
  labels: labelsSchema.optional(),

  type: z.enum(['server', 'client']).default('server').optional(),
  snis: z.array(z.string().min(1)),
  certificates: z.array(
    z
      .object({
        certificate: certificateSchema,
        key: certificateSchema,
      })
      .strict(),
  ),
  client: z
    .object({
      ca: certificateSchema,
      depth: z.number().int().min(0).default(1).optional(),
      skip_mtls_uri_regex: z.array(z.string()).min(1).optional(),
    })
    .strict(),
  ssl_protocols: z.array(z.enum(['TLSv1.1', 'TLSv1.2', 'TLSv1.3'])).max(3),
});

const pluginConfigSchema = z.object({
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  plugins: pluginsSchema.optional(),
});

const consumerSchema = z.object({
  username: z.string().min(1),
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  plugins: pluginsSchema.optional(),
});

const consumerGroupSchema = z.object({
  name: nameSchema,
  description: descriptionSchema.optional(),
  labels: labelsSchema.optional(),

  plugins: pluginsSchema,

  consumers: z.array(consumerSchema).optional(),
});

export const ConfigurationSchema = z.object({
  routes: z.array(routeSchema).optional(),
  services: z.array(serviceSchema).optional(),
  upstreams: z.array(refineUpstream(upstreamSchema)).optional(),
  ssls: z.array(sslSchema).optional(),
  plugin_configs: z.array(pluginConfigSchema).optional(),
  consumers: z.array(consumerSchema).optional(),
  consumer_groups: z.array(consumerGroupSchema).optional(),
  stream_routes: z.array(streamRouteSchema).optional(),
  global_rules: z.record(z.string(), z.record(z.string(), z.any())).optional(),
  plugin_metadata: z
    .record(z.string(), z.record(z.string(), z.any()))
    .optional(),
});

/* const res = configurationSchema.safeParse({
  services: [
    {
      name: 'str',
      description: '',
      labels: {
        ADC_TEST: '123',
        ADC_TSET: ['123', '234'],
      },
    },
  ],
} as z.infer<typeof configurationSchema>);

if (!res.success) {
  console.log((res as SafeParseError<unknown>).error);
} */
