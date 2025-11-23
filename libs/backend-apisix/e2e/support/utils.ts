import { Differ } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import { Listr, SilentRenderer } from 'listr2';
import { lastValueFrom, toArray } from 'rxjs';
import semver from 'semver';

import { BackendAPISIX } from '../../src';

export const runTask = async (tasks: Listr, ctx = {}) => {
  //@ts-expect-error just ignore
  tasks.renderer = new SilentRenderer();
  await tasks.run(ctx);
  return tasks.ctx.local;
};

export const syncEvents = async (
  backend: BackendAPISIX,
  events: Array<ADCSDK.Event> = [],
) => lastValueFrom(backend.sync(events).pipe(toArray()));

export const dumpConfiguration = async (backend: BackendAPISIX) =>
  lastValueFrom(backend.dump());

export const cleanup = async (backend: BackendAPISIX) =>
  syncEvents(backend, Differ.diff({}, await dumpConfiguration(backend)));

export const getDefaultValue = async (backend: BackendAPISIX) =>
  backend.defaultValue();

export const createEvent = (
  resourceType: ADCSDK.ResourceType,
  resourceName: string,
  resource: object,
  parentName?: string,
): ADCSDK.Event => ({
  type: ADCSDK.EventType.CREATE,
  resourceType,
  resourceName,
  resourceId:
    resourceType === ADCSDK.ResourceType.CONSUMER ||
    resourceType === ADCSDK.ResourceType.GLOBAL_RULE ||
    resourceType === ADCSDK.ResourceType.PLUGIN_METADATA
      ? resourceName
      : resourceType === ADCSDK.ResourceType.SSL
        ? ADCSDK.utils.generateId((resource as ADCSDK.SSL).snis.join(','))
        : ADCSDK.utils.generateId(
            parentName ? `${parentName}.${resourceName}` : resourceName,
          ),
  newValue: resource,
  parentId: parentName
    ? resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
      ? parentName
      : ADCSDK.utils.generateId(parentName)
    : undefined,
});

export const updateEvent = (
  resourceType: ADCSDK.ResourceType,
  resourceName: string,
  resource: object,
  parentName?: string,
): ADCSDK.Event => {
  const event = createEvent(resourceType, resourceName, resource, parentName);
  event.type = ADCSDK.EventType.UPDATE;
  return event;
};

export const deleteEvent = (
  resourceType: ADCSDK.ResourceType,
  resourceName: string,
  parentName?: string,
): ADCSDK.Event => ({
  type: ADCSDK.EventType.DELETE,
  resourceType,
  resourceName,
  resourceId:
    resourceType === ADCSDK.ResourceType.CONSUMER ||
    resourceType === ADCSDK.ResourceType.GLOBAL_RULE ||
    resourceType === ADCSDK.ResourceType.PLUGIN_METADATA
      ? resourceName
      : ADCSDK.utils.generateId(
          parentName ? `${parentName}.${resourceName}` : resourceName,
        ),
  parentId: parentName
    ? resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
      ? parentName
      : ADCSDK.utils.generateId(parentName)
    : undefined,
});

export const overrideEventResourceId = (
  event: ADCSDK.Event,
  resourceId: string,
  parentId?: string,
) => {
  event.resourceId = resourceId;
  if (parentId) event.parentId = parentId;
  return event;
};

export const sortResult = <T extends Record<string, any>>(
  result: Array<T>,
  field: string,
) => structuredClone(result).sort((a, b) => a[field].localeCompare(b[field]));

export const wait = (ms: number) =>
  new Promise((resolve) => setTimeout(resolve, ms));

type cond = boolean | (() => boolean);

export const conditionalDescribe = (
  cond: cond,
): typeof describe | typeof describe.skip => (cond ? describe : describe.skip);

export const conditionalIt = (cond: cond): typeof it | typeof it.skip =>
  cond ? it : it.skip;

export const semverCondition = (
  op: (v1: string | semver.SemVer, v2: string | semver.SemVer) => boolean,
  base: string,
  target = semver.coerce(process.env.BACKEND_APISIX_VERSION) ?? '0.0.0',
) => op(target, base);
