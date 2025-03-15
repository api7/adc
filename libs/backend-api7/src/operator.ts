import * as ADCSDK from '@api7/adc-sdk';
import { Axios, AxiosResponse } from 'axios';
import EventEmitter from 'node:events';
import { SemVer } from 'semver';

import { FromADC } from './transformer';

export interface OperatorOptions {
  client: Axios;
  version: SemVer;
  eventEmitter: EventEmitter;
  gatewayGroupName?: string;
  gatewayGroupId?: string;
}
export class Operator {
  private readonly client: Axios;
  private readonly eventEmitter: EventEmitter;

  constructor(private readonly opts: OperatorOptions) {
    this.client = opts.client;
    this.eventEmitter = opts.eventEmitter;
  }

  public async updateResource(event: ADCSDK.Event) {
    let resp: AxiosResponse<{ error_msg?: string }>;
    if (event.resourceType === ADCSDK.ResourceType.SERVICE) {
      // Create published service directly
      resp = await this.client.put(
        `/apisix/admin/services/${event.resourceId}`,
        this.fromADC(event),
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    } else if (event.resourceType === ADCSDK.ResourceType.ROUTE) {
      // Create route directly
      resp = await this.client.put(
        `/apisix/admin/routes/${event.resourceId}`,
        this.fromADC(event),
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    } else if (event.resourceType === ADCSDK.ResourceType.STREAM_ROUTE) {
      // Create stream route directly
      resp = await this.client.put(
        `/apisix/admin/stream_routes/${event.resourceId}`,
        this.fromADC(event),
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    } else if (event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL) {
      resp = await this.client.put(
        `/apisix/admin/consumers/${event.parentId}/credentials/${event.resourceId}`,
        this.fromADC(event),
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    } else {
      resp = await this.client.put(
        `/apisix/admin/${this.generateResourceTypeInAPI(event.resourceType)}/${event.resourceId}`,
        this.fromADC(event),
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    }

    if (resp?.data?.error_msg) throw new Error(resp.data.error_msg);
    return resp;
  }

  public async deleteResource(event: ADCSDK.Event) {
    let resp: AxiosResponse<{ error_msg?: string }>;
    if (event.resourceType === ADCSDK.ResourceType.SERVICE) {
      // Remove published service on the gateway group
      resp = await this.client.delete(
        `/apisix/admin/services/${event.resourceId}`,
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      //TODO logger
      this.emitDebugLog(
        resp,
        `Remove service "${event.resourceName}" on the gateway group ${this.opts.gatewayGroupName}`,
      );
    } else if (event.resourceType === ADCSDK.ResourceType.ROUTE) {
      resp = await this.client.delete(
        `/apisix/admin/routes/${event.resourceId}`,
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    } else if (event.resourceType === ADCSDK.ResourceType.STREAM_ROUTE) {
      resp = await this.client.delete(
        `/apisix/admin/stream_routes/${event.resourceId}`,
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
      //TODO logger task.output = buildReqAndRespDebugOutput(resp);
    } else if (event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL) {
      resp = await this.client.delete(
        `/apisix/admin/consumers/${event.parentId}/credentials/${event.resourceId}`,
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    } else {
      resp = await this.client.delete(
        `/apisix/admin/${this.generateResourceTypeInAPI(event.resourceType)}/${event.resourceId}`,
        {
          params: {
            gateway_group_id: this.opts.gatewayGroupId,
          },
          validateStatus: () => true,
        },
      );
      this.emitDebugLog(resp);
    }

    if (resp?.data?.error_msg) throw new Error(resp.data.error_msg);
    return resp;
  }

  private emitDebugLog(response: AxiosResponse, description?: string) {
    this.eventEmitter.emit(ADCSDK.BackendEventType.AXIOS_DEBUG, {
      response,
      description,
    } satisfies ADCSDK.BackendEventAxiosDebug);
  }

  private generateResourceTypeInAPI(resourceType: ADCSDK.ResourceType) {
    return resourceType !== ADCSDK.ResourceType.PLUGIN_METADATA
      ? `${resourceType}s`
      : ADCSDK.ResourceType.PLUGIN_METADATA;
  }

  private fromADC(event: ADCSDK.Event) {
    const fromADC = new FromADC();
    switch (event.resourceType) {
      case ADCSDK.ResourceType.CONSUMER:
        return fromADC.transformConsumer(event.newValue as ADCSDK.Consumer);
      case ADCSDK.ResourceType.GLOBAL_RULE:
        return {
          plugins: {
            [event.resourceId]: event.newValue,
          },
        };
      case ADCSDK.ResourceType.PLUGIN_METADATA:
        return event.newValue;
      case ADCSDK.ResourceType.SERVICE:
        (event.newValue as ADCSDK.Service).id = event.resourceId;
        return fromADC.transformService(event.newValue as ADCSDK.Service);
      case ADCSDK.ResourceType.ROUTE:
        (event.newValue as ADCSDK.Route).id = event.resourceId;
        return fromADC.transformRoute(
          event.newValue as ADCSDK.Route,
          event.parentId,
        );
      case ADCSDK.ResourceType.STREAM_ROUTE:
        (event.newValue as ADCSDK.StreamRoute).id = event.resourceId;
        return fromADC.transformStreamRoute(
          event.newValue as ADCSDK.StreamRoute,
          event.parentId,
        );
      case ADCSDK.ResourceType.SSL:
        (event.newValue as ADCSDK.SSL).id = event.resourceId;
        return fromADC.transformSSL(event.newValue as ADCSDK.SSL);
      case ADCSDK.ResourceType.CONSUMER_CREDENTIAL:
        (event.newValue as ADCSDK.ConsumerCredential).id = event.resourceId;
        return fromADC.transformConsumerCredential(
          event.newValue as ADCSDK.ConsumerCredential,
        );
    }
  }
}
