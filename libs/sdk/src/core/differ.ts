import type { Diff } from 'datum-diff';
import type { ZodObject, ZodRawShape } from 'zod';

import { FieldListType, ResourceType } from './resource';
import { type FieldMeta, differFieldRegistry } from './field-registry';
import {
  consumerGroupSchema,
  consumerSchema,
  routeSchema,
  serviceBaseSchema,
  sslSchema,
  streamRouteSchema,
} from './schema';
import type {
  Consumer,
  ConsumerCredential,
  ConsumerGroup,
  InternalConfiguration,
  Route,
  SSL,
  Service,
  StreamRoute,
  Upstream,
} from './schema';
import { utils } from '../utils';
import type { Plugin, ResourceFor } from '.';

export { FieldListType };

export enum EventType {
  CREATE = 'create',
  DELETE = 'delete',
  UPDATE = 'update',

  // Internal use only, the backend does not need to handle such event type
  ONLY_SUB_EVENTS = 'only_sub_events',
}

export type Event<T extends ResourceType = any> = {
  type: EventType;
  resourceId: string;
  resourceName: string;
  diff?: Array<Diff<ResourceFor<T>, object>>;

  resourceType: ResourceType;
  oldValue?: ResourceFor<T>;
  newValue?: ResourceFor<T>;

  // for nested events
  parentId?: string;
  subEvents?: Array<Event>;
};

export type ResourceName = string;
export type ResourceId = string;
export type ResourceDefaultValue = Partial<Record<ResourceType, object>>;
export type PluginDefaultValue = Record<string, Plugin>;
export type DefaultValue = {
  core?: ResourceDefaultValue;
  plugins?: PluginDefaultValue;
};

export const CollectionKind = {
  ARRAY: 'array',
  RECORD: 'record',
} as const;
export type CollectionKind = (typeof CollectionKind)[keyof typeof CollectionKind];

/** Read all differ-relevant field annotations from a Zod object schema's shape. */
export function readFieldMeta(schema: ZodObject<ZodRawShape>): Record<string, FieldMeta> {
  const result: Record<string, FieldMeta> = {};
  for (const [key, field] of Object.entries(schema.shape)) {
    const meta = differFieldRegistry.get(field as Parameters<typeof differFieldRegistry.get>[0]);
    if (meta?.listType) result[key] = meta;
  }
  return result;
}

export interface ResourceDifferMeta {
  /** Key on InternalConfiguration that holds this resource's collection. */
  configField: keyof InternalConfiguration | undefined;
  /** Whether the collection is an array (most types) or a Record (global_rules, plugin_metadata). */
  collectionKind: CollectionKind;
  /** Derive the display name used as resourceName in events. */
  getName: (item: unknown) => string;
  /** Compute or extract the ID for a local item (no server-assigned id yet). */
  generateId: (item: unknown, parentName?: string) => string;
  /** Whether to pass the parent resource name when generating IDs for child resources. */
  propagatesParentName?: boolean;
  /**
   * Resolve which ResourceType to use for default-value lookup.
   * Only needed for SERVICE, which may be a stream service.
   */
  resolveDefaultType?: (item: unknown) => ResourceType;
  /**
   * Per-field merge strategies, derived from schema .meta() annotations via readFieldMeta().
   * Do not populate this by hand — the schema is the source of truth.
   */
  fields: Record<string, FieldMeta>;
}

export const RESOURCE_DIFFER_META: Partial<Record<ResourceType, ResourceDifferMeta>> = {
  [ResourceType.SERVICE]: {
    configField: 'services',
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as Service).name,
    generateId: (r) => (r as Service).id ?? utils.generateId((r as Service).name),
    propagatesParentName: true,
    resolveDefaultType: (r) =>
      'stream_routes' in (r as object)
        ? ResourceType.INTERNAL_STREAM_SERVICE
        : ResourceType.SERVICE,
    fields: readFieldMeta(serviceBaseSchema),
  },

  [ResourceType.SSL]: {
    configField: 'ssls',
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as SSL).snis.join(','),
    generateId: (r) => (r as SSL).id ?? utils.generateId((r as SSL).snis.join(',')),
    fields: readFieldMeta(sslSchema),
  },

  [ResourceType.CONSUMER]: {
    configField: 'consumers',
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as Consumer).username,
    generateId: (r) => (r as Consumer).username,
    propagatesParentName: true,
    fields: readFieldMeta(consumerSchema),
  },

  [ResourceType.GLOBAL_RULE]: {
    configField: 'global_rules',
    collectionKind: CollectionKind.RECORD,
    getName: (key) => key as string,
    generateId: (key) => key as string,
    fields: {},
  },

  [ResourceType.PLUGIN_METADATA]: {
    configField: 'plugin_metadata',
    collectionKind: CollectionKind.RECORD,
    getName: (key) => key as string,
    generateId: (key) => key as string,
    fields: {},
  },

  [ResourceType.ROUTE]: {
    configField: 'routes',
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as Route).name,
    generateId: (r, parent) => {
      const res = r as Route;
      return res.id ?? utils.generateId(parent ? `${parent}.${res.name}` : res.name);
    },
    propagatesParentName: true,
    fields: readFieldMeta(routeSchema),
  },

  [ResourceType.STREAM_ROUTE]: {
    configField: 'stream_routes',
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as StreamRoute).name,
    generateId: (r, parent) => {
      const res = r as StreamRoute;
      return res.id ?? utils.generateId(parent ? `${parent}.${res.name}` : res.name);
    },
    fields: readFieldMeta(streamRouteSchema),
  },

  [ResourceType.CONSUMER_CREDENTIAL]: {
    configField: 'consumer_credentials',
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as ConsumerCredential).name,
    generateId: (r, parent) => {
      const res = r as ConsumerCredential;
      return res.id ?? utils.generateId(parent ? `${parent}.${res.name}` : res.name);
    },
    propagatesParentName: true,
    fields: {},
  },

  [ResourceType.UPSTREAM]: {
    configField: 'upstreams',
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as Upstream).name!,
    generateId: (r, parent) => {
      const res = r as Upstream & { id?: string };
      return res.id ?? utils.generateId(parent ? `${parent}.${res.name!}` : res.name!);
    },
    propagatesParentName: true,
    fields: {},
  },

  // CONSUMER_GROUP and PLUGIN_CONFIG are not yet in InternalConfiguration at top level
  // but appear as sub-resources; kept here for sub-event ordering and future extension.
  [ResourceType.CONSUMER_GROUP]: {
    configField: undefined,
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as ConsumerGroup).name,
    generateId: (r) =>
      (r as ConsumerGroup & { id?: string }).id ??
      utils.generateId((r as ConsumerGroup).name),
    fields: readFieldMeta(consumerGroupSchema),
  },

  [ResourceType.PLUGIN_CONFIG]: {
    configField: undefined,
    collectionKind: CollectionKind.ARRAY,
    getName: (r) => (r as { name: string }).name,
    generateId: (r) => {
      const res = r as { id?: string; name: string };
      return res.id ?? utils.generateId(res.name);
    },
    fields: {},
  },
};
