import * as ADCSDK from '@api7/adc-sdk';

import { DifferV3 } from '../differv3';

describe('Differ V3 - resources custom id', () => {
  it('should delete and create new resource when update resource id (with nested resource)', () => {
    const service1Name = 'Test Service 1';
    const service1Id = ADCSDK.utils.generateId(service1Name);
    const service2Name = 'Test Service 2';
    const service2Id = ADCSDK.utils.generateId(service2Name);
    const customId1 = 'custom-id-1';
    const customId2 = 'custom-id-2';
    expect(
      DifferV3.diff(
        {
          services: [
            {
              id: customId1,
              name: service1Name,
            },
            {
              name: service2Name,
            },
          ],
        },
        {
          services: [
            {
              id: service1Id,
              name: service1Name,
            },
            {
              id: customId2,
              name: service2Name,
            },
          ],
        },
      ),
    ).toEqual([
      {
        oldValue: { name: service1Name },
        resourceId: service1Id,
        resourceName: service1Name,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: { name: service2Name },
        resourceId: customId2,
        resourceName: service2Name,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.DELETE,
      },
      {
        newValue: { name: service1Name },
        resourceId: customId1,
        resourceName: service1Name,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.CREATE,
      },
      {
        newValue: { name: service2Name },
        resourceId: service2Id,
        resourceName: service2Name,
        resourceType: ADCSDK.ResourceType.SERVICE,
        type: ADCSDK.EventType.CREATE,
      },
    ]);
  });
});
