import { z } from 'zod';

import { FieldListType } from './resource';

export type FieldMeta =
  | { listType: typeof FieldListType.MAP; listMapKey: string; nested?: boolean; configKey?: string }
  | { listType: typeof FieldListType.OBJECT_MAP }
  | { listType: typeof FieldListType.ATOMIC; strip?: boolean }
  | { listType: typeof FieldListType.ARRAY; stripItemFields?: string[] };

/** Isolated registry for differ field metadata — not serialized into JSON schema output. */
export const differFieldRegistry = z.registry<FieldMeta>();

export function withDifferMeta<T extends z.ZodTypeAny>(schema: T, meta: FieldMeta): T {
  differFieldRegistry.add(schema, meta);
  return schema;
}
