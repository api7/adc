import * as ADCSDK from '@api7/adc-sdk';
import { z } from 'zod';

const SyncTask = z.strictObject({
  opts: z.looseObject({
    backend: z.string().min(1),
    server: z.string().min(1),
    token: z.string().min(1),
    lint: z.boolean().optional().default(true),
    includeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
    excludeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
    labelSelector: z.record(z.string(), z.string()).optional(),
    cacheKey: z.string(),
  }),
  config: z.looseObject({}),
});

export const SyncInput = z.strictObject({
  task: SyncTask,
});
export type SyncInputType = z.infer<typeof SyncInput>;
