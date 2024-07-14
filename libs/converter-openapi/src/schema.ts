import { z } from 'zod';

import { ExtKey } from './extension';

const xNameSchema = z.optional(z.string().min(1));
const xLabelsSchmea = z.optional(
  z.record(z.string(), z.union([z.string(), z.array(z.string())])),
);
const xPluginsSchema = z.optional(z.record(z.string(), z.any()));
const xDefaults = z.optional(z.record(z.string(), z.any()));
const pathOperationSchema = z.optional(
  z.object({
    [ExtKey.NAME]: xNameSchema,
    [ExtKey.LABELS]: xLabelsSchmea,
    [ExtKey.PLUGINS]: xPluginsSchema,
    // [ExtKey.PLUGIN_PREFIX]: ignore
    [ExtKey.SERVICE_DEFAULTS]: xDefaults,
    [ExtKey.UPSTREAM_DEFAULTS]: xDefaults,
    [ExtKey.ROUTE_DEFAULTS]: xDefaults,
    operationId: z.optional(z.string()),
    summary: z.optional(z.string()),
    description: z.optional(z.string()),
  }),
);
export const schema = z.object({
  info: z.object({
    title: z.string(),
    description: z.optional(z.string()),
  }),
  [ExtKey.NAME]: xNameSchema,
  [ExtKey.LABELS]: xLabelsSchmea,
  [ExtKey.PLUGINS]: xPluginsSchema,
  // [ExtKey.PLUGIN_PREFIX]: ignore
  [ExtKey.SERVICE_DEFAULTS]: xDefaults,
  [ExtKey.UPSTREAM_DEFAULTS]: xDefaults,
  [ExtKey.ROUTE_DEFAULTS]: xDefaults,
  servers: z
    .array(
      z.object({
        url: z.string().regex(/https?:\/\//g),
        variables: z.optional(
          z.record(
            z.string(),
            z.object({
              default: z.string(),
            }),
          ),
        ),
        [ExtKey.UPSTREAM_NODE_DEFAULTS]: xDefaults,
      }),
    )
    .min(1),
  paths: z.record(
    z.string(),
    z.object({
      [ExtKey.PLUGINS]: xPluginsSchema,
      // [ExtKey.PLUGIN_PREFIX]: ignore
      [ExtKey.SERVICE_DEFAULTS]: xDefaults,
      [ExtKey.UPSTREAM_DEFAULTS]: xDefaults,
      [ExtKey.ROUTE_DEFAULTS]: xDefaults,
      get: pathOperationSchema,
      put: pathOperationSchema,
      post: pathOperationSchema,
      delete: pathOperationSchema,
      options: pathOperationSchema,
      head: pathOperationSchema,
      patch: pathOperationSchema,
      trace: pathOperationSchema,
    }),
  ),
});
