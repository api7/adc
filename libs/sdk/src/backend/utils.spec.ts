import { EventType, ResourceType } from '../core';
import type { Event } from '../core';
import {
  MANAGED_BY_LABEL_KEY,
  MANAGED_BY_LABEL_VALUE,
  injectManagedByLabel,
} from './utils';

const buildEvent = (overrides: Partial<Event>): Event => ({
  type: EventType.CREATE,
  resourceId: 'id',
  resourceName: 'name',
  resourceType: ResourceType.ROUTE,
  newValue: { name: 'name' } as Event['newValue'],
  ...overrides,
});

describe('injectManagedByLabel', () => {
  it('merges managed-by=adc into newValue.labels on CREATE events', () => {
    const event = buildEvent({ type: EventType.CREATE });
    const [result] = injectManagedByLabel([event]);
    expect(result.newValue?.labels).toEqual({
      [MANAGED_BY_LABEL_KEY]: MANAGED_BY_LABEL_VALUE,
    });
  });

  it('merges managed-by=adc into newValue.labels on UPDATE events without dropping existing labels', () => {
    const event = buildEvent({
      type: EventType.UPDATE,
      newValue: { name: 'name', labels: { team: 'a' } } as Event['newValue'],
    });
    const [result] = injectManagedByLabel([event]);
    expect(result.newValue?.labels).toEqual({
      team: 'a',
      [MANAGED_BY_LABEL_KEY]: MANAGED_BY_LABEL_VALUE,
    });
  });

  it('does not modify DELETE events', () => {
    const event = buildEvent({
      type: EventType.DELETE,
      newValue: undefined,
      oldValue: { name: 'name' } as Event['oldValue'],
    });
    const [result] = injectManagedByLabel([event]);
    expect(result.newValue).toBeUndefined();
  });

  it('skips resource types without a labels field', () => {
    const globalRuleEvent = buildEvent({
      resourceType: ResourceType.GLOBAL_RULE,
      newValue: { 'limit-count': { count: 1 } } as unknown as Event['newValue'],
    });
    const pluginMetadataEvent = buildEvent({
      resourceType: ResourceType.PLUGIN_METADATA,
      newValue: { log_format: {} } as unknown as Event['newValue'],
    });
    const [result1, result2] = injectManagedByLabel([
      globalRuleEvent,
      pluginMetadataEvent,
    ]);
    expect(result1.newValue).not.toHaveProperty('labels');
    expect(result2.newValue).not.toHaveProperty('labels');
  });

  it('returns events unchanged when disabled', () => {
    const event = buildEvent({ type: EventType.CREATE });
    const [result] = injectManagedByLabel([event], false);
    expect(result.newValue?.labels).toBeUndefined();
  });

  it('does not touch the event.diff patch', () => {
    const diff = [{ path: ['name'], type: 'MODIFIED' as const }];
    const event = buildEvent({
      type: EventType.UPDATE,
      diff: diff as unknown as Event['diff'],
    });
    const [result] = injectManagedByLabel([event]);
    expect(result.diff).toBe(diff);
  });
});
