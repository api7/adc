import * as ADCSDK from '@api7/adc-sdk';
import axios, { Axios, CreateAxiosDefaults } from 'axios';
import { JSONSchema4 } from 'json-schema';
import { Listr, ListrTask } from 'listr2';
import { isEmpty, isNil } from 'lodash';
import { readFileSync } from 'node:fs';
import { AgentOptions, Agent as httpsAgent } from 'node:https';
import semver, { SemVer } from 'semver';

import { Fetcher } from './fetcher';
import { OperateContext, Operator } from './operator';
import { ToADC } from './transformer';
import * as typing from './typing';
import { buildReqAndRespDebugOutput } from './utils';

export class BackendAPI7 implements ADCSDK.Backend {
  private readonly client: Axios;
  private readonly gatewayGroup: string;
  private static logScope = ['API7'];

  private version: SemVer;
  private gatewayGroupId: string;
  private defaultValue: ADCSDK.DefaultValue;

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    const config: CreateAxiosDefaults = {
      baseURL: `${opts.server}`,
      headers: {
        'X-API-KEY': opts.token,
        'Content-Type': 'application/json',
        //'User-Agent': 'ADC/0.9.0-alpha.9 (ADC-API7-BACKEND)',
      },
    };

    if (opts.server.startsWith('https')) {
      const agentConfig: AgentOptions = {
        rejectUnauthorized: !opts?.tlsSkipVerify,
      };

      if (opts?.caCertFile) {
        agentConfig.ca = readFileSync(opts.caCertFile);
      }
      if (opts?.tlsClientCertFile) {
        agentConfig.cert = readFileSync(opts.tlsClientCertFile);
        agentConfig.key = readFileSync(opts.tlsClientKeyFile);
      }

      config.httpsAgent = new httpsAgent(agentConfig);
    }

    if (opts.timeout) config.timeout = opts.timeout;

    this.client = axios.create(config);
    this.gatewayGroup = opts.gatewayGroup;
  }

  public async ping() {
    await this.client.get('/api/gateway_groups');
  }

  public getResourceDefaultValueTask(): ListrTask {
    const mergeAllOf = (items: Array<JSONSchema4>) => {
      if (items.length < 2) return items[0];
      if (!items.every((item) => item.type === 'object')) return null;

      const first = items.shift();
      if (!first.properties) first.properties = {};
      return items.reduce((pv, cv) => {
        Object.entries(cv?.properties ?? {}).forEach(([key, val]) => {
          pv.properties[key] = val;
        });
        return pv;
      }, first);
    };
    const extractObjectDefault = (obj: JSONSchema4) => {
      if (obj.type !== 'object') return null;
      if (!obj.properties) return null;

      const defaults = Object.fromEntries(
        Object.entries(obj?.properties ?? {})
          .map(([key, field]) => {
            if (field.type === 'object')
              return [key, extractObjectDefault(field)];

            // For array nested object (e.g. service.upstream.nodes)
            if (field.type === 'array' && !Array.isArray(field.items)) {
              if (field.items.type === 'object')
                return [key, [extractObjectDefault(field.items)]];
            }

            return [key, field?.default];
          })
          .filter(
            ([, defaultValue]) =>
              !isNil(defaultValue) || !isEmpty(defaultValue),
          ),
      );

      return defaults;
    };
    return {
      enabled: (ctx) => isEmpty(ctx.defaultValue),
      task: async (ctx, task) => {
        if (!isEmpty(this.defaultValue)) {
          ctx.defaultValue = this.defaultValue;
          return;
        }
        const resp = await this.client.get<{
          value: ADCSDK.DefaultValue['core'];
        }>('/api/schema/core');
        task.output = buildReqAndRespDebugOutput(
          resp,
          'Get core resoruces schema',
        );

        ctx.defaultValue = this.defaultValue = {
          core: Object.fromEntries(
            Object.entries(resp.data.value).map(
              ([type, schema]: [string, JSONSchema4]) => {
                const transformer = (type: ADCSDK.ResourceType) => {
                  const toADC = new ToADC();
                  switch (type) {
                    case ADCSDK.ResourceType.ROUTE:
                      return toADC.transformRoute;
                    case ADCSDK.ResourceType.SERVICE:
                      return toADC.transformService;
                    case ADCSDK.ResourceType.SSL:
                      return toADC.transformSSL;
                    case ADCSDK.ResourceType.CONSUMER:
                      return toADC.transformConsumer;
                    default:
                      return <T>(res: T): T => res;
                  }
                };
                return [
                  type,
                  transformer(type as ADCSDK.ResourceType)(
                    extractObjectDefault(
                      schema.allOf ? mergeAllOf(schema.allOf) : schema,
                    ) ?? {},
                  ),
                ];
              },
            ),
          ),
        };
      },
    };
  }

  private getGatewayGroupIdTask(name: string): ListrTask {
    return {
      enabled: (ctx) => !ctx.gatewayGroupId,
      task: async (ctx, task) => {
        if (this.gatewayGroupId) {
          ctx.gatewayGroupId = this.gatewayGroupId;
          return;
        }

        const resp = await this.client.get<{ list: Array<{ id: string }> }>(
          '/api/gateway_groups',
          {
            params: {
              search: name,
            },
          },
        );
        task.output = buildReqAndRespDebugOutput(
          resp,
          `Get id of gateway group "${this.gatewayGroup}"`,
        );

        const gatewayGroups = resp?.data?.list;
        if (!gatewayGroups.length) {
          throw Error(`Gateway group ${this.gatewayGroup} does not exist`);
        }
        ctx.gatewayGroupId = this.gatewayGroupId = gatewayGroups[0].id;
      },
    };
  }

  private getAPI7VersionTask(): ListrTask {
    return {
      enabled: (ctx) => !ctx.api7Version,
      task: async (ctx, task) => {
        if (this.version) {
          ctx.api7Version = this.version;
          return;
        }

        const resp = await this.client.get<{ value: string }>('/api/version');
        task.output = buildReqAndRespDebugOutput(resp, `Get API7 version`);
        ctx.api7Version = this.version = semver.coerce(
          resp?.data.value || '0.0.0',
        );
      },
    };
  }

  public async dump(): Promise<Listr<{ remote: ADCSDK.Configuration }>> {
    const fetcher = new Fetcher(this.client, this.opts);

    return new Listr<{ remote: ADCSDK.Configuration }>(
      [
        this.getAPI7VersionTask(),
        this.getResourceDefaultValueTask(),
        this.getGatewayGroupIdTask(this.gatewayGroup),
        ...fetcher.allTask(),
      ],
      {
        //@ts-expect-error reorg renderer
        rendererOptions: { scope: BackendAPI7.logScope },
      },
    );
  }

  public async sync(): Promise<Listr> {
    const operator = new Operator(this.client, this.gatewayGroup);
    return new Listr<OperateContext>(
      [
        this.getAPI7VersionTask(),
        this.getGatewayGroupIdTask(this.gatewayGroup),
        this.syncPreprocessEventsTask(),
        {
          task: (ctx, task) =>
            task.newListr(
              ctx.diff.map((event) =>
                event.type === ADCSDK.EventType.DELETE
                  ? operator.deleteResource(event)
                  : operator.updateResource(event),
              ),
            ),
        },
        operator.publishService(),
      ],
      {
        //@ts-expect-error TODO reorg renderer
        rendererOptions: { scope: BackendAPI7.logScope },
      },
    );
  }

  // Preprocess events for sync:
  // 1. Events that attempt to remove routes but not for the purpose of
  //    republishing the service will be ignored.
  // 2. The service will at least be removed from the gateway group, i.e.,
  //    it will stop processing such traffic.
  //    Sometimes service templates cannot be removed because it is still
  ///   referenced by published services on other gateway groups.
  private syncPreprocessEventsTask(): ListrTask<OperateContext> {
    return {
      task: (ctx) => {
        const isRouteLike = (resourceType: ADCSDK.ResourceType) =>
          [
            ADCSDK.ResourceType.ROUTE,
            ADCSDK.ResourceType.STREAM_ROUTE,
          ].includes(resourceType);
        const deletedServiceIds = ctx.diff
          .filter(
            (item) =>
              item.resourceType === ADCSDK.ResourceType.SERVICE &&
              item.type === ADCSDK.EventType.DELETE,
          )
          .map((item) => item.resourceId);

        ctx.needPublishServices = Object.fromEntries<typing.Service | null>(
          ctx.diff
            .filter((item) => {
              // Include creation and update event types of services
              if (
                item.resourceType === ADCSDK.ResourceType.SERVICE &&
                item.type !== ADCSDK.EventType.DELETE
              )
                return true;

              // Include subroutes that have changed but the service itself has
              // not been deleted
              if (
                isRouteLike(item.resourceType) &&
                !deletedServiceIds.includes(item.parentId)
              )
                return true;

              return false;
            })
            .map((item) => [
              item.resourceType === ADCSDK.ResourceType.SERVICE
                ? item.resourceId
                : item.parentId,
              null,
            ]),
        );

        // Exclude those events we do not need
        ctx.diff = ctx.diff
          // Include route non deletion events
          .filter((item) => {
            if (!isRouteLike(item.resourceType)) return true;
            if (
              isRouteLike(item.resourceType) &&
              item.type !== ADCSDK.EventType.DELETE
            )
              return true;

            // Include routes removed just to change the service and republish it
            if (
              isRouteLike(item.resourceType) &&
              item.type === ADCSDK.EventType.DELETE &&
              !deletedServiceIds.includes(item.parentId)
            )
              return true;

            return false;
          });
      },
    };
  }
}
