export enum ResourceType {
  ROUTE = 'route',
  SERVICE = 'service',
  UPSTREAM = 'upstream',
  SSL = 'ssl',
  GLOBAL_RULE = 'global_rule',
  PLUGIN_CONFIG = 'plugin_config',
  PLUGIN_METADATA = 'plugin_metadata',
  CONSUMER = 'consumer',
  CONSUMER_GROUP = 'consumer_group',
  CONSUMER_CREDENTIAL = 'consumer_credential',
  STREAM_ROUTE = 'stream_route',

  // internal use only
  INTERNAL_STREAM_SERVICE = 'stream_service',
}

/** Merge strategies for resource fields, mirroring structured-merge-diff listType semantics. */
export const FieldListType = {
  /** Array of objects, identity by a declared key field. nested=true triggers sub-event diffing. */
  MAP: 'map',
  /** Record<string, V> — identity by property key (e.g. plugins). */
  OBJECT_MAP: 'objectMap',
  /** Treat the field as an opaque value; strip=true removes it before comparison. */
  ATOMIC: 'atomic',
  /** Plain array whose items need individual sub-field stripping before comparison. */
  ARRAY: 'array',
} as const;
export type FieldListType = (typeof FieldListType)[keyof typeof FieldListType];
