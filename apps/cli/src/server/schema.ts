import * as ADCSDK from '@api7/adc-sdk';
import { z } from 'zod';

import type { TlsMaterial } from './agent-pool';

const isPemLike = (value?: string) => !value || value.trim().startsWith('-----BEGIN');

// tlsClientCert/tlsClientKey must be provided together, and any provided PEM field
// must at least look like PEM content (full certificate/key parsing happens at the
// TLS layer when the connection is actually established).
const tlsCertKeyPaired = (o: TlsMaterial) => !!o.tlsClientCert === !!o.tlsClientKey;
const caCertIsPemLike = (o: TlsMaterial) => isPemLike(o.caCert);
const tlsClientCertIsPemLike = (o: TlsMaterial) => isPemLike(o.tlsClientCert);
const tlsClientKeyIsPemLike = (o: TlsMaterial) => isPemLike(o.tlsClientKey);

const tlsShape = {
  tlsSkipVerify: z.boolean().optional(),
  caCert: z.string().min(1).optional(),
  tlsClientCert: z.string().min(1).optional(),
  tlsClientKey: z.string().min(1).optional(),
};

const SyncTask = z.strictObject({
  opts: z
    .looseObject({
      backend: z.string().min(1),
      server: z.union([z.url().min(1), z.array(z.url().min(1))]),
      token: z.string().min(1),
      lint: z.boolean().optional().default(true),
      includeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
      excludeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
      labelSelector: z.record(z.string(), z.string()).optional(),
      cacheKey: z.string(),
      bypassCache: z.boolean().optional().default(false),
      ...tlsShape,
    })
    .refine(tlsCertKeyPaired, {
      error: 'tlsClientCert and tlsClientKey must be provided together',
      path: ['tlsClientKey'],
    })
    .refine(caCertIsPemLike, {
      error: 'caCert does not look like a PEM-encoded certificate',
      path: ['caCert'],
    })
    .refine(tlsClientCertIsPemLike, {
      error: 'tlsClientCert does not look like a PEM-encoded certificate',
      path: ['tlsClientCert'],
    })
    .refine(tlsClientKeyIsPemLike, {
      error: 'tlsClientKey does not look like a PEM-encoded key',
      path: ['tlsClientKey'],
    }),
  config: z.looseObject({}),
});

export const SyncInput = z.strictObject({
  task: SyncTask,
});
export type SyncInputType = z.infer<typeof SyncInput>;

const ValidateTask = z.strictObject({
  opts: z
    .looseObject({
      backend: z.string().min(1),
      server: z.union([z.url().min(1), z.array(z.url().min(1))]),
      token: z.string().min(1),
      lint: z.boolean().optional().default(true),
      includeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
      excludeResourceType: z.array(z.enum(ADCSDK.ResourceType)).optional(),
      labelSelector: z.record(z.string(), z.string()).optional(),
      cacheKey: z.string(),
      ...tlsShape,
    })
    .refine(tlsCertKeyPaired, {
      error: 'tlsClientCert and tlsClientKey must be provided together',
      path: ['tlsClientKey'],
    })
    .refine(caCertIsPemLike, {
      error: 'caCert does not look like a PEM-encoded certificate',
      path: ['caCert'],
    })
    .refine(tlsClientCertIsPemLike, {
      error: 'tlsClientCert does not look like a PEM-encoded certificate',
      path: ['tlsClientCert'],
    })
    .refine(tlsClientKeyIsPemLike, {
      error: 'tlsClientKey does not look like a PEM-encoded key',
      path: ['tlsClientKey'],
    }),
  config: z.looseObject({}),
});

export const ValidateInput = z.strictObject({
  task: ValidateTask,
});
export type ValidateInputType = z.infer<typeof ValidateInput>;
