import * as ADCSDK from '@api7/adc-sdk';
import { Listr, SilentRenderer } from 'listr2';

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
) => {
  return runTask(await backend.sync(), { diff: events });
};

export const dumpConfiguration = async (backend: BackendAPI7) => {
  const ctx = { remote: {} };
  await runTask(await backend.dump(), ctx);
  return ctx.remote;
};

export const getDefaultValue = async (backend: BackendAPI7) => {
  const ctx = { defaultValue: {} };
  await runTask(new Listr(backend.getResourceDefaultValueTask()), ctx);
  return ctx.defaultValue;
};

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
  parentId: parentName ? ADCSDK.utils.generateId(parentName) : undefined,
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
  parentId: parentName ? ADCSDK.utils.generateId(parentName) : undefined,
});

type cond = boolean | (() => boolean);

export const conditionalDescribe = (cond: cond) =>
  cond ? describe : describe.skip;

export const conditionalIt = (cond: cond) => (cond ? it : it.skip);

export const semverCondition = (
  op: (v1: string, v2: string) => boolean,
  target: string,
  base = process.env.BACKEND_API7_VERSION ?? '0.0.0',
) => op(target, base);
