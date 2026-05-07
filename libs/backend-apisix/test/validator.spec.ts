/**
 * Unit tests for the APISIX Validator.
 *
 * NOTE: These tests are fully mocked — they test that the Validator correctly
 * maps APISIX validate errors back to ADC Event objects by asserting on the
 * `event` field in `BackendValidationError`. No real HTTP connections are made;
 * `axios.post` is stubbed via `vi.spyOn` to return a simulated 400 response.
 */
import * as ADCSDK from '@api7/adc-sdk';
import axios, { AxiosError } from 'axios';
import { Subject } from 'rxjs';

import { Validator } from '../src/validator';

/**
 * Helper to create an AxiosError that mimics a 400 validation response.
 */
const createAxios400Error = (data: Record<string, unknown>): AxiosError => {
  const error = new AxiosError(
    'Request failed with status code 400',
    AxiosError.ERR_BAD_REQUEST,
  );
  error.response = {
    status: 400,
    statusText: 'Bad Request',
    headers: {},
    data,
    config: {} as any,
  };
  return error;
};

describe('Validator', () => {
  const createEvent = (
    resourceType: ADCSDK.ResourceType,
    resourceName: string,
    parentId?: string,
  ): ADCSDK.Event => ({
    type: ADCSDK.EventType.CREATE,
    resourceType,
    resourceId: ADCSDK.utils.generateId(resourceName),
    resourceName,
    newValue: { name: resourceName },
    ...(parentId ? { parentId } : {}),
  });

  it('should embed event in validation errors for routes', async () => {
    const parentId = ADCSDK.utils.generateId('httpbin.org');
    const events = [
      createEvent(ADCSDK.ResourceType.SERVICE, 'httpbin.org'),
      createEvent(ADCSDK.ResourceType.ROUTE, 'get-anything', parentId),
    ];

    const client = axios.create();
    vi.spyOn(client, 'post').mockRejectedValue(
      createAxios400Error({
        error_msg: 'Configuration validation failed',
        errors: [
          {
            resource_type: 'routes',
            index: 0,
            error:
              'does not match schema due to: Error at "/methods/0": value is not one of the allowed values',
          },
        ],
      }),
    );

    const validator = new Validator({
      client,
      eventSubject: new Subject<ADCSDK.BackendEvent>(),
    });

    const result = await validator.validate(events);

    expect(result.success).toBe(false);
    expect(result.errorMessage).toBe('Configuration validation failed');
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0].resource_type).toBe('routes');
    expect(result.errors[0].resource_name).toBe('get-anything');
    expect(result.errors[0].event).toBeDefined();
    expect(result.errors[0].event!.resourceType).toBe(
      ADCSDK.ResourceType.ROUTE,
    );
    expect(result.errors[0].event!.resourceName).toBe('get-anything');
    expect(result.errors[0].event!.parentId).toBe(parentId);
    expect(result.errors[0].event!.type).toBe(ADCSDK.EventType.CREATE);
    expect(result.errors[0].event!.newValue).toMatchObject({
      name: 'get-anything',
    });
  });

  it('should embed event in validation errors for services', async () => {
    const events = [
      createEvent(ADCSDK.ResourceType.SERVICE, 'httpbin.org'),
      createEvent(ADCSDK.ResourceType.ROUTE, 'get-anything', 'some-parent'),
    ];

    const client = axios.create();
    vi.spyOn(client, 'post').mockRejectedValue(
      createAxios400Error({
        error_msg: 'Configuration validation failed',
        errors: [
          {
            resource_type: 'services',
            index: 0,
            error: 'does not match schema due to: plugins validation failed',
          },
        ],
      }),
    );

    const validator = new Validator({
      client,
      eventSubject: new Subject<ADCSDK.BackendEvent>(),
    });

    const result = await validator.validate(events);

    expect(result.errors).toHaveLength(1);
    expect(result.errors[0].resource_type).toBe('services');
    expect(result.errors[0].resource_name).toBe('httpbin.org');
    expect(result.errors[0].event).toBeDefined();
    expect(result.errors[0].event!.resourceType).toBe(
      ADCSDK.ResourceType.SERVICE,
    );
    expect(result.errors[0].event!.resourceName).toBe('httpbin.org');
  });

  it('should return success when no validation errors', async () => {
    const events = [createEvent(ADCSDK.ResourceType.SERVICE, 'httpbin.org')];

    const client = axios.create();
    vi.spyOn(client, 'post').mockResolvedValue({
      status: 200,
      data: {},
    });

    const validator = new Validator({
      client,
      eventSubject: new Subject<ADCSDK.BackendEvent>(),
    });

    const result = await validator.validate(events);

    expect(result.success).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('should handle multiple errors with correct event mapping', async () => {
    const parentId = ADCSDK.utils.generateId('my-service');
    const events = [
      createEvent(ADCSDK.ResourceType.SERVICE, 'my-service'),
      createEvent(ADCSDK.ResourceType.ROUTE, 'route-a', parentId),
      createEvent(ADCSDK.ResourceType.ROUTE, 'route-b', parentId),
      createEvent(ADCSDK.ResourceType.CONSUMER, 'user1'),
    ];

    const client = axios.create();
    vi.spyOn(client, 'post').mockRejectedValue(
      createAxios400Error({
        error_msg: 'Configuration validation failed',
        errors: [
          {
            resource_type: 'routes',
            index: 0,
            error: 'error on route-a',
          },
          {
            resource_type: 'routes',
            index: 1,
            error: 'error on route-b',
          },
          {
            resource_type: 'consumers',
            index: 0,
            error: 'error on user1',
          },
        ],
      }),
    );

    const validator = new Validator({
      client,
      eventSubject: new Subject<ADCSDK.BackendEvent>(),
    });

    const result = await validator.validate(events);

    expect(result.errors).toHaveLength(3);

    // First error: routes[0] → route-a
    expect(result.errors[0].resource_name).toBe('route-a');
    expect(result.errors[0].event!.resourceName).toBe('route-a');
    expect(result.errors[0].event!.parentId).toBe(parentId);

    // Second error: routes[1] → route-b
    expect(result.errors[1].resource_name).toBe('route-b');
    expect(result.errors[1].event!.resourceName).toBe('route-b');
    expect(result.errors[1].event!.parentId).toBe(parentId);

    // Third error: consumers[0] → user1
    expect(result.errors[2].resource_name).toBe('user1');
    expect(result.errors[2].event!.resourceName).toBe('user1');
    expect(result.errors[2].event!.parentId).toBeUndefined();
  });

  it('should handle events without matching error index gracefully', async () => {
    const events = [createEvent(ADCSDK.ResourceType.SERVICE, 'my-service')];

    const client = axios.create();
    vi.spyOn(client, 'post').mockRejectedValue(
      createAxios400Error({
        errors: [
          {
            resource_type: 'unknown_type',
            index: 0,
            error: 'some error',
          },
        ],
      }),
    );

    const validator = new Validator({
      client,
      eventSubject: new Subject<ADCSDK.BackendEvent>(),
    });

    const result = await validator.validate(events);

    expect(result.errors).toHaveLength(1);
    // Unknown resource type: event should be undefined, no crash
    expect(result.errors[0].event).toBeUndefined();
  });
});
