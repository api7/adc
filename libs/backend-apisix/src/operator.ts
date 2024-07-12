import * as ADCSDK from '@api7/adc-sdk';
import { Axios } from 'axios';
import { ListrTask } from 'listr2';

import { FromADC } from './transformer';
import * as typing from './typing';
import {
  buildReqAndRespDebugOutput,
  capitalizeFirstLetter,
  resourceTypeToAPIName,
} from './utils';

export interface OperateContext {
  diff: Array<ADCSDK.Event>;
  gatewayGroupId: string;
  needPublishServices: Record<string, typing.Service | null>;
}
type OperateTask = ListrTask<OperateContext>;

export class Operator {
  constructor(private readonly client: Axios) {}

  public updateResource(event: ADCSDK.Event): OperateTask {
    return {
      title: this.generateTaskName(event),
      task: async (ctx, task) => {
        const resp = await this.client.put(
          `/apisix/admin/${resourceTypeToAPIName(event.resourceType)}/${event.resourceId}`,
          this.fromADC(event),
          {
            validateStatus: () => true,
          },
        );
        task.output = buildReqAndRespDebugOutput(resp);

        if (resp.data?.error_msg) throw new Error(resp.data.error_msg);
        // [200, 201].includes(resp.status);
      },
    };
  }

  public deleteResource(event: ADCSDK.Event): OperateTask {
    return {
      title: this.generateTaskName(event),
      task: async (ctx, task) => {
        const resp = await this.client.delete(
          `/apisix/admin/${resourceTypeToAPIName(event.resourceType)}/${event.resourceId}`,
          {
            validateStatus: () => true,
          },
        );
        task.output = buildReqAndRespDebugOutput(resp);

        // If the resource does not exist, it is not an error for the delete operation
        if (resp.status === 404) return;
        if (resp.data?.error_msg) throw new Error(resp.data.error_msg);
        if (resp.data?.deleted <= 0)
          throw new Error(
            `Unexpected number of deletions of resources: ${resp.data?.deleted}`,
          );
        // [200, 404].includes(resp.status);
      },
    };
  }

  private generateTaskName(event: ADCSDK.Event) {
    return `${capitalizeFirstLetter(
      event.type,
    )} ${event.resourceType}: "${event.resourceName}"`;
  }

  private fromADC(event: ADCSDK.Event) {
    const fromADC = new FromADC();
    switch (event.resourceType) {
      case ADCSDK.ResourceType.CONSUMER:
        return fromADC.transformConsumer(event.newValue as ADCSDK.Consumer);
      case ADCSDK.ResourceType.CONSUMER_GROUP:
        return fromADC.transformConsumerGroup(
          event.newValue as ADCSDK.ConsumerGroup,
        )[0];
      case ADCSDK.ResourceType.GLOBAL_RULE:
        return {
          plugins: {
            [event.resourceId]: event.newValue,
          },
        };
      case ADCSDK.ResourceType.PLUGIN_METADATA:
        return event.newValue;
      case ADCSDK.ResourceType.ROUTE: {
        const route = fromADC.transformRoute(event.newValue as ADCSDK.Route);
        if (event.parentId) route.service_id = event.parentId;
        return route;
      }
      case ADCSDK.ResourceType.SERVICE:
        return fromADC.transformService(event.newValue as ADCSDK.Service)[0];
      case ADCSDK.ResourceType.SSL:
        return fromADC.transformSSL(event.newValue as ADCSDK.SSL);
      case ADCSDK.ResourceType.STREAM_ROUTE: {
        const route = fromADC.transformStreamRoute(
          event.newValue as ADCSDK.StreamRoute,
        );
        if (event.parentId) route.service_id = event.parentId;
        return route;
      }
    }
  }
}
