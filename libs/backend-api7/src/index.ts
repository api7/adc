import * as ADCSDK from '@api7/adc-sdk';
import axios, { type AxiosInstance, type CreateAxiosDefaults } from 'axios';
import { JSONSchema4 } from 'json-schema';
import { isEmpty, isNil } from 'lodash';
import { readFileSync } from 'node:fs';
import {
  Agent as httpAgent,
  AgentOptions as httpAgentOptions,
} from 'node:http';
import {
  Agent as httpsAgent,
  AgentOptions as httpsAgentOptions,
} from 'node:https';
import { Subject, forkJoin, from, switchMap } from 'rxjs';
import semver, { SemVer } from 'semver';

import { Fetcher } from './fetcher';
import { Operator } from './operator';
import { ToADC } from './transformer';
import * as typing from './typing';

export class BackendAPI7 implements ADCSDK.Backend {
  private readonly client: AxiosInstance;
  private readonly gatewayGroupName?: string;
  private static logScope = ['API7'];
  private readonly subject = new Subject<ADCSDK.BackendEvent>();

  private _version?: SemVer;
  private gatewayGroupId?: string;
  private innerDefaultValue?: ADCSDK.DefaultValue;

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    const keepAlive: httpAgentOptions = {
      keepAlive: true,
      keepAliveMsecs: 60000,
      maxSockets: 256, // per host
    };
    const config: CreateAxiosDefaults = {
      baseURL: `${opts.server}`,
      headers: {
        'X-API-KEY': opts.token,
        'Content-Type': 'application/json',
      },
      httpAgent: new httpAgent(keepAlive),
    };

    if (opts.server.startsWith('https')) {
      const agentConfig: httpsAgentOptions = {
        ...keepAlive,
        rejectUnauthorized: !opts?.tlsSkipVerify,
      };

      if (opts?.caCertFile) {
        agentConfig.ca = readFileSync(opts.caCertFile);
      }
      if (opts?.tlsClientCertFile) {
        agentConfig.cert = readFileSync(opts.tlsClientCertFile);
        if (opts?.tlsClientKeyFile) {
          agentConfig.key = readFileSync(opts.tlsClientKeyFile);
        }
      }

      config.httpsAgent = new httpsAgent(agentConfig);
    }

    if (opts.timeout) config.timeout = opts.timeout;

    this.client = axios.create(config);
    this.gatewayGroupName = opts.gatewayGroup;
  }

  public metadata() {
    return {
      logScope: BackendAPI7.logScope,
    };
  }

  public async ping() {
    await this.client.get('/api/gateway_groups');
  }

  public async version() {
    if (this._version) return this._version;

    const resp = await this.client.get<{ value: string }>('/api/version');
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: resp, description: 'Get API7 version' },
    });

    return (this._version =
      resp?.data?.value === 'dev'
        ? semver.coerce('999.999.999')!
        : semver.coerce(resp?.data?.value) || semver.coerce('0.0.0')!);
  }

  public async defaultValue() {
    if (this.innerDefaultValue) return this.innerDefaultValue;
    const mergeAllOf = (items: Array<JSONSchema4>) => {
      if (items.length < 2) return items[0];
      if (!items.some((item) => item.type === 'object')) return {};

      const first = items.shift() || {};
      if (!first?.properties) first.properties = {};
      return items.reduce((pv = {}, cv) => {
        Object.entries(cv?.properties ?? {}).forEach(([key, val]) => {
          if (!pv.properties) pv.properties = {};
          pv.properties[key] = val;
        });
        return pv;
      }, first);
    };
    const extractObjectDefault = (
      obj: JSONSchema4,
    ): Record<string, any> | null => {
      if (obj.type !== 'object') return null;
      if (!obj.properties) return null;

      const defaults = Object.fromEntries(
        Object.entries(obj?.properties ?? {})
          .map(([key, field]) => {
            if (field.type === 'object')
              return [key, extractObjectDefault(field)];

            // For array nested object (e.g. service.upstream.nodes)
            if (field.type === 'array' && !Array.isArray(field.items)) {
              if (field?.items?.type === 'object')
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
    const resp = await this.client.get<{
      value: ADCSDK.DefaultValue['core'];
    }>('/api/schema/core');
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: resp, description: 'Get core resoruces schema' },
    });

    // Fix upstream schema from 3.5 to 3.7
    if (!resp.data?.value?.upstream && resp.data?.value?.service)
      resp.data.value.upstream = {
        ...(resp.data?.value?.service as any)?.properties?.upstream,
        type: 'object',
      };

    const toADC = new ToADC();
    return (this.innerDefaultValue = {
      core: Object.fromEntries(
        (
          Object.entries(resp.data.value || {}) as Array<
            [ADCSDK.ResourceType, JSONSchema4]
          >
        ).map(([type, schema]) => {
          const data =
            extractObjectDefault(
              schema.allOf ? mergeAllOf(schema.allOf) : schema,
            ) ?? {};
          switch (type) {
            case ADCSDK.ResourceType.ROUTE:
              return [type, toADC.transformRoute(data as typing.Route)];
            case ADCSDK.ResourceType.INTERNAL_STREAM_SERVICE:
            case ADCSDK.ResourceType.SERVICE:
              return [type, toADC.transformService(data as typing.Service)];
            case ADCSDK.ResourceType.SSL:
              return [type, toADC.transformSSL(data as typing.SSL)];
            case ADCSDK.ResourceType.CONSUMER:
              return [type, toADC.transformConsumer(data as typing.Consumer)];
            case ADCSDK.ResourceType.UPSTREAM:
              return [type, toADC.transformUpstream(data as typing.Upstream)];
            default:
              return [type, data];
          }
        }),
      ),
    } as ADCSDK.DefaultValue);
  }

  private async getGatewayGroupId() {
    if (this.gatewayGroupId) return this.gatewayGroupId;
    if (this.opts?.token?.startsWith('a7adm-')) return undefined;

    const resp = await this.client.get<{ list: Array<{ id: string }> }>(
      '/api/gateway_groups',
      {
        params: {
          search: this.gatewayGroupName,
          name: this.gatewayGroupName,
        },
      },
    );
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: {
        response: resp,
        description: `Get id of gateway group "${this.gatewayGroupName}"`,
      },
    });

    const gatewayGroups = resp?.data?.list;
    if (!gatewayGroups?.length)
      throw Error(`Gateway group "${this.gatewayGroupName}" does not exist`);
    return (this.gatewayGroupId = gatewayGroups[0].id);
  }

  public dump() {
    return forkJoin([
      from(this.version()),
      from(this.getGatewayGroupId()),
      from(this.defaultValue()),
    ]).pipe<ADCSDK.Configuration>(
      switchMap(([version, gatewayGroupId]) => {
        const fetcher = new Fetcher({
          client: this.client,
          version: version,
          eventSubject: this.subject,
          backendOpts: this.opts,
          gatewayGroupName: this.gatewayGroupName,
          gatewayGroupId: gatewayGroupId,
        });
        return fetcher.dump();
      }),
    );
  }

  public sync(
    events: Array<ADCSDK.Event>,
    opts: ADCSDK.BackendSyncOptions = { exitOnFailure: true },
  ) {
    return forkJoin([
      from(this.version()),
      from(this.getGatewayGroupId()),
      from(this.defaultValue()),
    ]).pipe(
      switchMap(([version, gatewayGroupId]) => {
        return new Operator({
          client: this.client,
          version,
          eventSubject: this.subject,
          gatewayGroupId,
          gatewayGroupName: this.gatewayGroupName,
        }).sync(events, opts);
      }),
    );
  }

  public on(
    eventType: keyof typeof ADCSDK.BackendEventType,
    cb: (...args: any[]) => void,
  ) {
    return this.subject.subscribe(({ type, event }) => {
      if (eventType === type) cb(event);
    });
  }
}
