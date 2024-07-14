import { Diff } from 'deep-diff';

import { Plugin, Resource, ResourceType } from '.';

export enum EventType {
  CREATE = 'create',
  DELETE = 'delete',
  UPDATE = 'update',

  // Internal use only, the backend does not need to handle such event type
  ONLY_SUB_EVENTS = 'only_sub_events',
}

export type Event = {
  type: EventType;
  resourceId: string;
  resourceName: string;
  diff?: Array<Diff<unknown>>;

  resourceType: ResourceType;
  oldValue?: Resource;
  newValue?: Resource;

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
