import { z } from 'zod';

import { ExtKey } from './extension';

const xNameSchema = z.string().min(1).optional();
const xLabelsSchmea = z
  .record(z.string(), z.union([z.string(), z.array(z.string())]))
  .optional();
const xPluginsSchema = z.record(z.string(), z.any()).optional();
const xDefaults = z.record(z.string(), z.any()).optional();
const pathOperationSchema = z
  .looseObject({
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
  })
  .optional();
export const schema = z.looseObject({
  info: z.looseObject({
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
      z.looseObject({
        url: z
          .string()
          .regex(
            /https?:\/\//g,
            'The URL must be start with "https://" or "http://"',
          ),
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
    z.looseObject({
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
