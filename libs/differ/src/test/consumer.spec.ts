import * as ADCSDK from '@api7/adc-sdk';

import { DifferV3 } from '../differv3.js';

describe('Differ V3 - consumer', () => {
  it('should create/update/delete consumer credentials', () => {
    const consumerName = 'jack';
    const changeme = 'changeme';
    expect(
      DifferV3.diff(
        {
          consumers: [
            {
              username: consumerName,
              credentials: [
                {
                  name: ADCSDK.EventType.CREATE,
                  type: 'key-auth',
                  config: {
                    key: consumerName,
                  },
                },
                {
                  name: ADCSDK.EventType.UPDATE,
                  type: 'basic-auth',
                  config: {
                    username: consumerName,
                    password: `${changeme}.new`,
                  },
                },
              ],
            },
          ],
        },
        {
          consumers: [
            {
              username: consumerName,
              credentials: [
                {
                  id: ADCSDK.utils.generateId(
                    `${consumerName}.${ADCSDK.EventType.UPDATE}`,
                  ),
                  name: ADCSDK.EventType.UPDATE,
                  type: 'basic-auth',
                  config: {
                    username: consumerName,
                    password: changeme,
                  },
                },
                {
                  id: ADCSDK.utils.generateId(
                    `${consumerName}.${ADCSDK.EventType.DELETE}`,
                  ),
                  name: ADCSDK.EventType.DELETE,
                  type: 'jwt-auth',
                  config: {
                    key: consumerName,
                    secret: changeme,
                  },
                },
              ],
            },
          ],
        },
      ),
    ).toEqual([
      {
        oldValue: {
          config: { key: consumerName, secret: changeme },
          name: ADCSDK.EventType.DELETE,
          type: 'jwt-auth',
        },
        parentId: consumerName,
        resourceId: ADCSDK.utils.generateId(
          `${consumerName}.${ADCSDK.EventType.DELETE}`,
        ),
        resourceName: ADCSDK.EventType.DELETE,
        resourceType: ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
        type: ADCSDK.EventType.DELETE,
      },
      {
        newValue: {
          config: { key: consumerName },
          name: ADCSDK.EventType.CREATE,
          type: 'key-auth',
        },
        parentId: consumerName,
        resourceId: ADCSDK.utils.generateId(
          `${consumerName}.${ADCSDK.EventType.CREATE}`,
        ),
        resourceName: ADCSDK.EventType.CREATE,
        resourceType: ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
        type: ADCSDK.EventType.CREATE,
      },
      {
        diff: [
          {
            kind: 'E',
            lhs: changeme,
            path: ['config', 'password'],
            rhs: `${changeme}.new`,
          },
        ],
        newValue: {
          config: { password: `${changeme}.new`, username: consumerName },
          name: ADCSDK.EventType.UPDATE,
          type: 'basic-auth',
        },
        oldValue: {
          config: { password: changeme, username: consumerName },
          name: ADCSDK.EventType.UPDATE,
          type: 'basic-auth',
        },
        parentId: consumerName,
        resourceId: ADCSDK.utils.generateId(
          `${consumerName}.${ADCSDK.EventType.UPDATE}`,
        ),
        resourceName: ADCSDK.EventType.UPDATE,
        resourceType: ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
        type: ADCSDK.EventType.UPDATE,
      },
    ]);
  });

  it('should delete consumer credentials when consumer is deleting', () => {
    const consumerName = 'jack';
    const changeme = 'changeme';
    expect(
      DifferV3.diff(
        {
          consumers: [],
        },
        {
          consumers: [
            {
              username: consumerName,
              credentials: [
                {
                  id: ADCSDK.utils.generateId(
                    `${consumerName}.${ADCSDK.EventType.DELETE}`,
                  ),
                  name: ADCSDK.EventType.DELETE,
                  type: 'jwt-auth',
                  config: { key: consumerName, secret: changeme },
                },
              ],
            },
          ],
        },
      ),
    ).toEqual([
      {
        oldValue: {
          credentials: [
            {
              config: { key: consumerName, secret: changeme },
              id: '86a4d8fbeda9c3de3705a7ba087a8ec741cd1c17',
              name: ADCSDK.EventType.DELETE,
              type: 'jwt-auth',
            },
          ],
          username: consumerName,
        },
        resourceId: consumerName,
        resourceName: consumerName,
        resourceType: ADCSDK.ResourceType.CONSUMER,
        type: ADCSDK.EventType.DELETE,
      },
      {
        oldValue: {
          config: { key: consumerName, secret: changeme },
          name: ADCSDK.EventType.DELETE,
          type: 'jwt-auth',
        },
        parentId: consumerName,
        resourceId: ADCSDK.utils.generateId(
          `${consumerName}.${ADCSDK.EventType.DELETE}`,
        ),
        resourceName: ADCSDK.EventType.DELETE,
        resourceType: ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
        type: ADCSDK.EventType.DELETE,
      },
    ]);
  });
});
