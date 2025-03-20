import * as ADCSDK from '@api7/adc-sdk';
import { Listr, SilentRenderer } from 'listr2';
import { lastValueFrom, toArray } from 'rxjs';
import * as semver from 'semver';

import { BackendAPI7 } from '../../src';

export const runTask = async (tasks: Listr, ctx = {}) => {
  // add sync delay
  if (Array.isArray(tasks.task)) {
    const delayedTasks = tasks.task.reduce((pv, cv) => {
      pv.push(cv, {
        task: async () => new Promise((resolve) => setTimeout(resolve, 20)),
      });
      return pv;
    }, []);
    tasks = new Listr(delayedTasks, { concurrent: false });
  }

  //@ts-expect-error just ignore
  tasks.renderer = new SilentRenderer();
  await tasks.run(ctx);
  return tasks.ctx.local;
};

export const syncEvents = async (
  backend: BackendAPI7,
  events: Array<ADCSDK.Event> = [],
) => lastValueFrom(backend.sync(events).pipe(toArray()));

export const dumpConfiguration = async (backend: BackendAPI7) =>
  lastValueFrom(backend.dump());

export const getDefaultValue = async (backend: BackendAPI7) =>
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

export const sortResult = <T>(result: Array<T>, field: string) =>
  structuredClone(result).sort((a, b) => a[field].localeCompare(b[field]));

type cond = boolean | (() => boolean);

export const conditionalDescribe = (cond: cond) =>
  cond ? describe : describe.skip;

export const conditionalIt = (cond: cond) => (cond ? it : it.skip);

export const semverCondition = (
  op: (v1: string | semver.SemVer, v2: string | semver.SemVer) => boolean,
  base: string,
  target = semver.coerce(process.env.BACKEND_API7_VERSION) ?? '0.0.0',
) => op(target, base);
