import * as ADCSDK from '@api7/adc-sdk';
import { z } from 'zod';

const tlsSkipVerify = z.boolean().optional();

// PEM-encoded CA certificate (or bundle) used to verify the backend,
// instead of the system trust store.
const caCert = z
  .string()
  .min(1)
  .refine((cert) => cert.includes('-----BEGIN CERTIFICATE-----'), {
    error: 'caCert must be a PEM-encoded certificate',
  })
  .optional();

const SyncTask = z.strictObject({
  opts: z.looseObject({
    backend: z.string().min(1),
    server: z.union([z.url().min(1), z.array(z.url().min(1))]),
    token: z.string().min(1),
    lint: z.boolean().optional().default(true),
    includeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
    excludeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
    labelSelector: z.record(z.string(), z.string()).optional(),
    cacheKey: z.string(),
    bypassCache: z.boolean().optional().default(false),
    tlsSkipVerify,
    caCert,
  }),
  config: z.looseObject({}),
});

export const SyncInput = z.strictObject({
  task: SyncTask,
});
export type SyncInputType = z.infer<typeof SyncInput>;

const ValidateTask = z.strictObject({
  opts: z.looseObject({
    backend: z.string().min(1),
    server: z.union([z.url().min(1), z.array(z.url().min(1))]),
    token: z.string().min(1),
    lint: z.boolean().optional().default(true),
    includeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
    excludeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
    labelSelector: z.record(z.string(), z.string()).optional(),
    cacheKey: z.string(),
    tlsSkipVerify,
    caCert,
  }),
  config: z.looseObject({}),
});

export const ValidateInput = z.strictObject({
  task: ValidateTask,
});
export type ValidateInputType = z.infer<typeof ValidateInput>;
