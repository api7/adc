import * as ADCSDK from '@api7/adc-sdk';
import { Axios, AxiosResponse } from 'axios';
import { ListrTask } from 'listr2';
import { size } from 'lodash';
import { SemVer, gte as semVerGTE } from 'semver';

import { FromADC } from './transformer';
import * as typing from './typing';
import { buildReqAndRespDebugOutput, capitalizeFirstLetter } from './utils';

export interface OperateContext {
  api7Version: SemVer;
  diff: Array<ADCSDK.Event>;
  gatewayGroupId: string;
  needPublishServices: Record<string, typing.Service | null>;
}
type OperateTask = ListrTask<OperateContext>;

export class Operator {
  private readonly client: Axios;
  private readonly gatewayGroupName: string;
  private readonly version: SemVer;

  constructor(opts: {
    client: Axios;
    gatewayGroupName: string;
    version: SemVer;
  }) {
    this.client = opts.client;
    this.gatewayGroupName = opts.gatewayGroupName;
    this.version = opts.version;
  }

  public updateResource(event: ADCSDK.Event): OperateTask {
    return {
      title: this.generateTaskName(event),
      task: async (ctx, task) => {
        let resp: AxiosResponse<{ error_msg?: string }>;
        if (event.resourceType === ADCSDK.ResourceType.SERVICE) {
          ctx.needPublishServices[event.resourceId] = this.fromADC(
            event,
          ) as typing.Service;

          // Create a service template instead of create published service directly
          resp = await this.client.put(
            `/api/services/template/${event.resourceId}`,
            this.fromADC(event),
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(resp);
        } else if (event.resourceType === ADCSDK.ResourceType.ROUTE) {
          // Create a route template instead of create route directly
          resp = await this.client.put(
            `/api/routes/template/${event.resourceId}`,
            this.fromADC(event),
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(resp);
        } else if (event.resourceType === ADCSDK.ResourceType.STREAM_ROUTE) {
          // Create a stream route template instead of create stream route directly
          resp = await this.client.put(
            `/api/stream_routes/template/${event.resourceId}`,
            this.fromADC(event),
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(resp);
        } else if (
          event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL &&
          semVerGTE(ctx.api7Version, '3.2.15')
        ) {
          resp = await this.client.put(
            `/apisix/admin/consumers/${event.parentId}/credentials/${event.resourceId}`,
            this.fromADC(event),
            {
              params: {
                gateway_group_id: ctx.gatewayGroupId,
              },
              validateStatus: () => true,
            },
          );
          task.output = buildReqAndRespDebugOutput(resp);
        } else {
          resp = await this.client.put(
            `/apisix/admin/${this.generateResourceTypeInAPI(event.resourceType)}/${event.resourceId}`,
            this.fromADC(event),
            {
              params: {
                gateway_group_id: ctx.gatewayGroupId,
              },
              validateStatus: () => true,
            },
          );
          task.output = buildReqAndRespDebugOutput(resp);
        }

        if (resp?.data?.error_msg) throw new Error(resp.data.error_msg);
        // [200, 201].includes(resp.status);
      },
    };
  }

  public deleteResource(event: ADCSDK.Event): OperateTask {
    return {
      title: this.generateTaskName(event),
      task: async (ctx, task) => {
        // If the resource type is Service, we should first disable it
        if (event.resourceType === ADCSDK.ResourceType.SERVICE) {
          // Modify the status of published service on gateway group
          let resp = await this.client.patch(
            `/api/gateway_groups/${ctx.gatewayGroupId}/services/${event.resourceId}/runtime_configuration`,
            [
              {
                op: 'replace',
                path: '/status',
                value: 0,
              },
            ],
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(
            resp,
            `Disable service "${event.resourceName}" on the gateway group ${this.gatewayGroupName}`,
          );

          // Remove disabled service on the gateway group
          resp = await this.client.delete(
            `/api/gateway_groups/${ctx.gatewayGroupId}/services/${event.resourceId}`,
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(
            resp,
            `Remove service "${event.resourceName}" on the gateway group ${this.gatewayGroupName}`,
          );

          // Check for other references to the service template and do not attempt
          // to delete the service template if there are other references
          interface UnknownList {
            list: Array<unknown>;
          }
          resp = await this.client.get<UnknownList>(
            `/api/services/${event.resourceId}/published_services`,
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(
            resp,
            `Get publish record for service "${event.resourceName}"`,
          );
          if ((resp.data as UnknownList)?.list?.length > 0) {
            task.output = JSON.stringify({
              type: 'debug',
              messages: [
                'Service template delete is skipped: it is still used by other gateway groups',
              ],
            });
            return;
          }

          resp = await this.client.delete(
            `/api/services/template/${event.resourceId}`,
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(
            resp,
            `Remove service template "${event.resourceName}"`,
          );
          return;
        } else if (event.resourceType === ADCSDK.ResourceType.ROUTE) {
          const resp = await this.client.delete(
            `/api/routes/template/${event.resourceId}`,
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(resp);
          return;
        } else if (event.resourceType === ADCSDK.ResourceType.STREAM_ROUTE) {
          const resp = await this.client.delete(
            `/api/stream_routes/template/${event.resourceId}`,
            { validateStatus: () => true },
          );
          task.output = buildReqAndRespDebugOutput(resp);
          return;
        } else if (
          event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL &&
          semVerGTE(ctx.api7Version, '3.2.15')
        ) {
          const resp = await this.client.delete(
            `/apisix/admin/consumers/${event.parentId}/credentials/${event.resourceId}`,
            {
              params: {
                gateway_group_id: ctx.gatewayGroupId,
              },
              validateStatus: () => true,
            },
          );
          task.output = buildReqAndRespDebugOutput(resp);
          return;
        }

        const resp = await this.client.delete(
          `/apisix/admin/${this.generateResourceTypeInAPI(event.resourceType)}/${event.resourceId}`,
          {
            params: {
              gateway_group_id: ctx.gatewayGroupId,
            },
            validateStatus: () => true,
          },
        );
        task.output = buildReqAndRespDebugOutput(resp);

        if (resp?.data?.error_msg) throw new Error(resp.data.error_msg);
        // [200, 404].includes(resp.status);
      },
    };
  }

  public publishService(): OperateTask {
    return {
      title: 'Publish services',
      skip: (ctx) => {
        if (!ctx.needPublishServices || size(ctx.needPublishServices) <= 0)
          return 'No services to be published';
      },
      task: async (ctx, task) => {
        const services = ctx.needPublishServices;

        for (const [serviceId, service] of Object.entries(services)) {
          if (!service) {
            const resp = await this.client.get<{ value: typing.Service }>(
              `/api/gateway_groups/${ctx.gatewayGroupId}/services/${serviceId}`,
            );
            task.output = buildReqAndRespDebugOutput(
              resp,
              `Fill in nil service, id ${serviceId}`,
            );
            services[serviceId] = resp?.data?.value;
          }
        }

        const resp = await this.client.post(
          '/api/services/publish',
          {
            create_new_version: true,
            gateway_group_id: ctx.gatewayGroupId,
            services: Object.values(services).map(
              (service: typing.Service) => ({
                ...service,
                version: new Date().getTime().toString(),
                hosts: service?.hosts?.length ? service.hosts : undefined,
              }),
            ),
          },
          {
            validateStatus: () => true,
          },
        );
        task.output = buildReqAndRespDebugOutput(resp, 'Publish all services');

        if (resp?.data?.error_msg) throw new Error(resp.data.error_msg);
        // [200, 201].includes(resp.status);
      },
    };
  }

  private generateTaskName(event: ADCSDK.Event) {
    return `${capitalizeFirstLetter(
      event.type,
    )} ${event.resourceType}: "${event.resourceName}"`;
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
      case ADCSDK.ResourceType.ROUTE: {
        (event.newValue as ADCSDK.Route).id = event.resourceId;
        const route = fromADC.transformRoute(
          event.newValue as ADCSDK.Route,
          event.parentId,
        );
        if (!semVerGTE(this.version, '3.2.16')) delete route.vars;
        return route;
      }
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
