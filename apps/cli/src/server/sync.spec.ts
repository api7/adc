import * as ADCSDK from '@api7/adc-sdk';

import {
  generateOutput,
  generateOutputForAPISIXStandalone,
  simplifyEvent,
} from './sync';

// resourceType/resourceId/resourceName/parentId: ADCSDK.Event's own field names
// (libs/sdk/src/core/differ.ts), the wire format every /sync consumer (AIC included)
// parses. TypeScript's structural typing means a stray snake_case field here would
// only be caught by a runtime check like this, not the type system alone, since these
// tests build plain object literals the same way real events are shaped on the wire.
// Cast rather than fully typed: these only exercise the envelope's own keys, the
// resource bodies under oldValue/newValue don't need to satisfy any specific
// resource type's shape.
const route = {
  type: ADCSDK.EventType.UPDATE,
  resourceId: 'r1',
  resourceName: 'test-route',
  resourceType: ADCSDK.ResourceType.ROUTE,
  oldValue: { id: 'r1' },
  newValue: { id: 'r1', uri: '/foo' },
  parentId: 'svc1',
} as unknown as ADCSDK.Event;

const service = {
  type: ADCSDK.EventType.CREATE,
  resourceId: 'svc1',
  resourceName: 'test-service',
  resourceType: ADCSDK.ResourceType.SERVICE,
  newValue: {},
} as unknown as ADCSDK.Event;

const syncResult = (
  event: ADCSDK.Event,
  overrides: Partial<ADCSDK.BackendSyncResult> = {},
): ADCSDK.BackendSyncResult => ({
  success: true,
  event,
  ...overrides,
});

describe('simplifyEvent', () => {
  it('keeps the camelCase envelope and strips oldValue/newValue/diff/subEvents', () => {
    const simplified = simplifyEvent(route);

    expect(simplified).toStrictEqual({
      type: ADCSDK.EventType.UPDATE,
      resourceId: 'r1',
      resourceName: 'test-route',
      resourceType: ADCSDK.ResourceType.ROUTE,
      parentId: 'svc1',
    });
    for (const key of ['resource_type', 'resource_id', 'resource_name', 'parent_id']) {
      expect(simplified).not.toHaveProperty(key);
    }
    for (const key of ['oldValue', 'newValue', 'diff', 'subEvents']) {
      expect(simplified).not.toHaveProperty(key);
    }
  });

  it('omits parentId rather than nulling it when the event has none', () => {
    const simplified = simplifyEvent(service);
    expect(simplified).not.toHaveProperty('parentId');
    expect(simplified.type).toBe(ADCSDK.EventType.CREATE);
  });
});

describe('generateOutput', () => {
  it('embeds the camelCase event envelope in both success and failed entries', () => {
    const success = syncResult(service);
    const failed = syncResult(route, {
      success: false,
      error: new Error('bad config'),
    });

    const output = generateOutput([[success, failed], [success], [failed]]);

    expect(output.success[0].event).toMatchObject({
      resourceType: ADCSDK.ResourceType.SERVICE,
      resourceId: 'svc1',
    });
    expect(output.failed[0].event).toMatchObject({
      resourceType: ADCSDK.ResourceType.ROUTE,
      resourceId: 'r1',
    });
    for (const event of [output.success[0].event, output.failed[0].event]) {
      expect(event).not.toHaveProperty('resource_type');
      expect(event).not.toHaveProperty('resource_id');
    }
  });
});

describe('generateOutputForAPISIXStandalone', () => {
  it('embeds the camelCase event envelope in its success entries', () => {
    const result = syncResult(service);
    const output = generateOutputForAPISIXStandalone(
      [service],
      [[result], [result], []],
    );

    expect(output.success[0].event).toMatchObject({
      resourceType: ADCSDK.ResourceType.SERVICE,
      resourceId: 'svc1',
    });
    expect(output.success[0].event).not.toHaveProperty('resource_type');
    expect(output.success[0].event).not.toHaveProperty('resource_id');
  });
});
