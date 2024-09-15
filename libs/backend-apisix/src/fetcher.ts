import * as ADCSDK from '@api7/adc-sdk';
import { Axios } from 'axios';
import { ListrTask } from 'listr2';

import { ToADC } from './transformer';
import * as typing from './typing';
import { buildReqAndRespDebugOutput, resourceTypeToAPIName } from './utils';

type FetchTask = ListrTask<{
  remote: ADCSDK.Configuration;
  apisixResources?: typing.Resources;
}>;

export class Fetcher {
  constructor(private readonly client: Axios) {}

  public fetch(): Array<FetchTask> {
    return [
      // Initialize context
      {
        task: (ctx) => {
          ctx.apisixResources = {};
        },
      },
      ...this.allTask(),
      {
        title: 'Reorganize resources',
        task: (_, task) => task.newListr(this.toADC()),
      },
    ];
  }

  private allTask(): Array<FetchTask> {
    return Object.values(ADCSDK.ResourceType)
      .filter(
        (resourceType) =>
          ![
            ADCSDK.ResourceType.INTERNAL_STREAM_SERVICE,
            ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
          ].includes(resourceType), // ignore internal only types
      )
      .map((resourceType): [ADCSDK.ResourceType, string] => [
        resourceType,
        resourceTypeToAPIName(resourceType),
      ])
      .map(
        ([resourceType, apiName]): FetchTask => ({
          title: `Fetch ${apiName}`,
          task: async (ctx, task) => {
            const resp = await this.client.get<{
              list: Array<{
                key: string;
                value: any;
                createdIndex: number;
                modifiedIndex: number;
              }>;
              total: number;
            }>(`/apisix/admin/${apiName}`, {
              validateStatus: () => true,
            });
            task.output = buildReqAndRespDebugOutput(resp, `Get ${apiName}`);
            if (
              resourceType === ADCSDK.ResourceType.STREAM_ROUTE &&
              resp.status === 400
            )
              return;

            // resourceType === ADCSDK.ResourceType.GLOBAL_RULE ||
            if (resourceType === ADCSDK.ResourceType.PLUGIN_METADATA) {
              ctx.apisixResources[ADCSDK.ResourceType.PLUGIN_METADATA] =
                Object.fromEntries(
                  resp.data?.list.map((item) => [
                    item.key.split('/').pop(),
                    ADCSDK.utils.recursiveOmitUndefined(item.value),
                  ]),
                );
            } else if (resourceType === ADCSDK.ResourceType.GLOBAL_RULE) {
              ctx.apisixResources[ADCSDK.ResourceType.GLOBAL_RULE] =
                Object.fromEntries(
                  resp.data?.list.map((item) => {
                    const pluginName = item.key.split('/').pop();
                    return [
                      pluginName,
                      ADCSDK.utils.recursiveOmitUndefined(
                        item.value?.plugins?.[pluginName] ?? {},
                      ),
                    ];
                  }),
                );
            } else {
              ctx.apisixResources[resourceType] = resp.data?.list.map(
                (item) => item.value,
              );
            }
          },
        }),
      );
  }

  private toADC(): Array<FetchTask> {
    const toADC = new ToADC();
    return [
      {
        title: 'Move plugin templates to route',
        task: (ctx) => {
          const resources = ctx.apisixResources;
          const pluginConfigIdMap = Object.fromEntries(
            resources?.plugin_config?.map((item) => [item.id, item]),
          );
          resources.route = resources?.route?.map((item) => {
            if (item.plugin_config_id)
              item.plugins = pluginConfigIdMap[item.plugin_config_id].plugins;
            return item;
          });
        },
      },
      {
        title: 'Move upstreams to service or route',
        task: (ctx) => {
          const resources = ctx.apisixResources;
          const upstreamIdMap = Object.fromEntries(
            resources?.upstream?.map((item) => [
              item.id,
              toADC.transformUpstream(item),
            ]),
          );
          resources.route = resources?.route?.map((item) => {
            if (item.upstream_id)
              item.upstream = upstreamIdMap[item.upstream_id];
            return item;
          });
          resources.service = resources?.service?.map((item) => {
            if (item.upstream_id)
              item.upstream = upstreamIdMap[item.upstream_id];
            return item;
          });
        },
      },
      {
        title: 'Move routes and stream_routes to service',
        task: (ctx) => {
          const resources = ctx.apisixResources;
          const serviceIdMap = Object.fromEntries(
            resources?.service?.map((item) => [
              item.id,
              toADC.transformService(item),
            ]),
          );
          resources?.route?.forEach((item) => {
            const route = toADC.transformRoute(item);
            if (item.service_id) {
              if (!serviceIdMap[item.service_id]) return; //TODO error report
              if (!serviceIdMap[item.service_id].routes)
                serviceIdMap[item.service_id].routes = [];
              serviceIdMap[item.service_id].routes.push(route);
            }
          });
          resources?.stream_route?.forEach((item) => {
            const route = toADC.transformStreamRoute(item);
            if (item.service_id) {
              if (!serviceIdMap[item.service_id]) return; //TODO error report
              if (!serviceIdMap[item.service_id].stream_routes)
                serviceIdMap[item.service_id].stream_routes = [];
              serviceIdMap[item.service_id].stream_routes.push(route);
            }
          });
          ctx.remote.services = Object.values(serviceIdMap).map((item) =>
            ADCSDK.utils.recursiveOmitUndefined({
              ...item,
              routes: item?.routes?.length > 0 ? item.routes : undefined,
              stream_routes:
                item?.stream_routes?.length > 0
                  ? item.stream_routes
                  : undefined,
            }),
          );
        },
      },
      {
        task: (ctx) => {
          const resources = ctx.apisixResources;
          ctx.remote = {
            ...ctx.remote,
            ssls: resources?.ssl?.map(toADC.transformSSL),
            consumers: resources?.consumer?.map((item) =>
              toADC.transformConsumer(item, true),
            ),
            global_rules: resources[ADCSDK.ResourceType.GLOBAL_RULE],
            plugin_metadata: resources[ADCSDK.ResourceType.PLUGIN_METADATA],
          };
        },
      },
    ];
  }
}
