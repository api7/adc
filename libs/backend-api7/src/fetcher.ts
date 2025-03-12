import * as ADCSDK from '@api7/adc-sdk';
import { Axios } from 'axios';
import { ListrTask } from 'listr2';
import { isEmpty } from 'lodash';
import { SemVer } from 'semver';

import { ToADC } from './transformer';
import * as typing from './typing';
import { buildReqAndRespDebugOutput } from './utils';

type FetchTaskContext = {
  api7Version: SemVer;
  gatewayGroupId?: string;
  remote: ADCSDK.Configuration;
};
type FetchTask = ListrTask<FetchTaskContext>;

export class Fetcher {
  private readonly toADC = new ToADC();

  constructor(
    private readonly client: Axios,
    private readonly backendOpts: ADCSDK.BackendOptions,
  ) {}

  public listServices(): FetchTask {
    return {
      title: 'Fetch services',
      skip: this.isSkip([ADCSDK.ResourceType.SERVICE]),
      task: async (ctx, task) => {
        const resp = await this.client.get<typing.ListResponse<typing.Service>>(
          `/apisix/admin/services`,
          { params: this.attachLabelSelector(this.attachGatewayGroup(ctx)) },
        );
        task.output = buildReqAndRespDebugOutput(resp, 'Get services');

        const services = resp?.data?.list;
        const fetchRoutes = services.map(async (service) => {
          const params = this.attachGatewayGroup(ctx, {
            service_id: service.id,
          });
          if (service.type === 'http') {
            const resp = await this.client.get<
              typing.ListResponse<typing.Route>
            >(`/apisix/admin/routes`, { params });
            task.output = buildReqAndRespDebugOutput(
              resp,
              `Get routes in service "${service.name}"`,
            );
            service.routes = resp?.data?.list;
          } else {
            const resp = await this.client.get<
              typing.ListResponse<typing.StreamRoute>
            >(`/apisix/admin/stream_routes`, { params });
            task.output = buildReqAndRespDebugOutput(
              resp,
              `Get stream routes in service "${service.name}"`,
            );
            service.stream_routes = resp?.data?.list;
          }
          return service;
        });
        await Promise.all(fetchRoutes);

        ctx.remote.services = services.map((item) =>
          this.toADC.transformService(item),
        );
      },
    };
  }

  public listConsumers(): FetchTask {
    return {
      title: 'Fetch consumers',
      skip: this.isSkip([ADCSDK.ResourceType.CONSUMER]),
      task: async (ctx, task) => {
        const resp = await this.client.get<{ list: Array<typing.Consumer> }>(
          '/apisix/admin/consumers',
          { params: this.attachLabelSelector(this.attachGatewayGroup(ctx)) },
        );
        task.output = buildReqAndRespDebugOutput(resp, 'Get consumers');

        const consumers = resp?.data?.list;

        const fetchCredentials = consumers.map(async (consumer) => {
          const resp = await this.client.get<{
            list: Array<typing.ConsumerCredential>;
          }>(`/apisix/admin/consumers/${consumer.username}/credentials`, {
            // In the current design, the consumer's credentials are not filtered
            // using labels because nested labels filters can be misleading. Even
            // if labels set for the consumer, the labels filter is not attached.
            params: this.attachGatewayGroup(ctx),
          });
          task.output = buildReqAndRespDebugOutput(
            resp,
            `Get credentials of consumer "${consumer.username}"`,
          );
          consumer.credentials = resp?.data?.list;
        });
        await Promise.all(fetchCredentials);

        ctx.remote.consumers = consumers?.map((item) =>
          this.toADC.transformConsumer(item),
        );
      },
    };
  }

  public listSSLs(): FetchTask {
    return {
      title: 'Fetch ssls',
      skip: this.isSkip([ADCSDK.ResourceType.SSL]),
      task: async (ctx, task) => {
        const resp = await this.client.get<{ list: Array<typing.SSL> }>(
          '/apisix/admin/ssls',
          { params: this.attachLabelSelector(this.attachGatewayGroup(ctx)) },
        );
        task.output = buildReqAndRespDebugOutput(resp, 'Get ssls');

        ctx.remote.ssls = resp?.data?.list?.map((item) =>
          this.toADC.transformSSL(item),
        );
      },
    };
  }

  public listGlobalRules(): FetchTask {
    return {
      title: 'Fetch global rules',
      skip: this.isSkip([ADCSDK.ResourceType.GLOBAL_RULE]),
      task: async (ctx, task) => {
        const resp = await this.client.get<{ list: Array<typing.GlobalRule> }>(
          '/apisix/admin/global_rules',
          { params: this.attachLabelSelector(this.attachGatewayGroup(ctx)) },
        );
        task.output = buildReqAndRespDebugOutput(resp, 'Get global rules');

        ctx.remote.global_rules = this.toADC.transformGlobalRule(
          resp?.data?.list ?? [],
        );
      },
    };
  }

  public listMetadatas(): FetchTask {
    return {
      title: 'Fetch plugin metadata',
      skip: this.isSkip([ADCSDK.ResourceType.PLUGIN_METADATA]),
      task: async (ctx, task) => {
        const resp = await this.client.get<{
          value: ADCSDK.Plugins;
        }>('/apisix/admin/plugin_metadata', {
          params: this.attachLabelSelector(this.attachGatewayGroup(ctx)),
        });
        task.output = buildReqAndRespDebugOutput(resp, 'Get plugin metadata');

        ctx.remote.plugin_metadata = this.toADC.transformPluginMetadatas(
          resp.data.value,
        );
      },
    };
  }

  public allTask() {
    return [
      this.listServices(),
      this.listConsumers(),
      this.listSSLs(),
      this.listGlobalRules(),
      this.listMetadatas(),
    ];
  }

  private isSkip(
    requiredTypes: Array<ADCSDK.ResourceType>,
  ): () => string | undefined {
    return () => {
      const msg = 'excluded by resource type filters';
      if (
        this.backendOpts?.includeResourceType &&
        !isEmpty(this.backendOpts.includeResourceType) &&
        !requiredTypes.some((item) =>
          this.backendOpts.includeResourceType.includes(item),
        )
      ) {
        return msg;
      } else if (
        this.backendOpts?.excludeResourceType &&
        !isEmpty(this.backendOpts.excludeResourceType) &&
        requiredTypes.every((item) =>
          this.backendOpts.excludeResourceType.includes(item),
        )
      ) {
        return msg;
      }
    };
  }

  private attachLabelSelector(
    params: Record<string, string> = {},
  ): Record<string, string> {
    if (this.backendOpts?.labelSelector)
      Object.entries(this.backendOpts.labelSelector).forEach(([key, value]) => {
        params[`labels[${key}]`] = value;
      });
    return params;
  }

  private attachGatewayGroup(
    ctx: FetchTaskContext,
    params: Record<string, string> = {},
  ) {
    if (ctx.gatewayGroupId) params.gateway_group_id = ctx.gatewayGroupId;
    return params;
  }
}
