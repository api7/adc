import * as ADCSDK from '@api7/adc-sdk';
import * as compose from 'docker-compose';
import { Listr, SilentRenderer } from 'listr2';
import { lastValueFrom, toArray } from 'rxjs';
import semver from 'semver';

import { BackendAPISIXStandalone as BackendAPISIX } from '../../src';

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

export const sortResult = <T extends Record<string, string>>(
  result: Array<T>,
  field: string,
) => structuredClone(result).sort((a, b) => a[field].localeCompare(b[field]));

export const wait = (ms: number) =>
  new Promise((resolve) => setTimeout(resolve, ms));

type cond = boolean | (() => boolean);

export const conditionalDescribe = (cond: cond): typeof describe =>
  cond ? describe : (describe.skip as typeof describe);

export const conditionalIt = (cond: cond): typeof it =>
  cond ? it : (it.skip as typeof it);

export const semverCondition = (
  op: (v1: string | semver.SemVer, v2: string | semver.SemVer) => boolean,
  base: string,
  target = semver.coerce(process.env.BACKEND_APISIX_VERSION) ?? '0.0.0',
) => op(target, base);

export const restartAPISIX = () =>
  compose.restartAll({
    cwd: 'libs/backend-apisix-standalone/e2e/assets',
  });
