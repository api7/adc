import * as ADCSDK from '@api7/adc-sdk';
import axios, { Axios, CreateAxiosDefaults } from 'axios';
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

export class BackendAPI7 implements ADCSDK.Backend {
  private readonly client: Axios;
  private readonly gatewayGroupName: string;
  private static logScope = ['API7'];
  private readonly subject = new Subject<ADCSDK.BackendEvent>();

  private version?: SemVer;
  private gatewayGroupId?: string;
  private innerDefaultValue: ADCSDK.DefaultValue;

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    const keepAlive: httpAgentOptions = {
      keepAlive: true,
      keepAliveMsecs: 60000,
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
        agentConfig.key = readFileSync(opts.tlsClientKeyFile);
      }

      config.httpsAgent = new httpsAgent(agentConfig);
    }

    if (opts.timeout) config.timeout = opts.timeout;

    this.client = axios.create(config);
    this.gatewayGroupName = opts.gatewayGroup;
  }

  public async ping() {
    await this.client.get('/api/gateway_groups');
  }

  public async defaultValue() {
    if (this.innerDefaultValue) return this.innerDefaultValue;
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
    const resp = await this.client.get<{
      value: ADCSDK.DefaultValue['core'];
    }>('/api/schema/core');
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: resp, description: 'Get core resoruces schema' },
    });

    return (this.innerDefaultValue = {
      core: Object.fromEntries(
        Object.entries(resp.data.value).map(
          ([type, schema]: [string, JSONSchema4]) => {
            const transformer = (type: ADCSDK.ResourceType) => {
              const toADC = new ToADC();
              switch (type) {
                case ADCSDK.ResourceType.ROUTE:
                  return toADC.transformRoute;
                case ADCSDK.ResourceType.INTERNAL_STREAM_SERVICE:
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

  private async getVersion() {
    if (this.version) return this.version;

    const resp = await this.client.get<{ value: string }>('/api/version');
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: resp, description: 'Get API7 version' },
    });

    return (this.version =
      resp?.data?.value === 'dev'
        ? semver.coerce('999.999.999')
        : semver.coerce(resp?.data?.value) || semver.coerce('0.0.0'));
  }

  public dump() {
    return forkJoin([
      from(this.getVersion()),
      from(this.defaultValue()),
      from(this.getGatewayGroupId()),
    ]).pipe<ADCSDK.Configuration>(
      switchMap(([version, , gatewayGroupId]) => {
        const fetcher = new Fetcher({
          client: this.client,
          version: version,
          eventSubject: this.subject,
          backendOpts: this.opts,
          gatewayGroupName: this.gatewayGroupName,
          gatewayGroupId: gatewayGroupId,
        });
        return fetcher.allTask();
      }),
    );
  }

  public sync(
    events: Array<ADCSDK.Event>,
    opts: { exitOnFailed: boolean } = { exitOnFailed: true },
  ) {
    return forkJoin([
      from(this.getVersion()),
      from(this.defaultValue()),
      from(this.getGatewayGroupId()),
    ]).pipe(
      switchMap(([version, , gatewayGroupId]) => {
        return new Operator({
          client: this.client,
          version,
          eventSubject: this.subject,
          gatewayGroupId,
          gatewayGroupName: this.gatewayGroupName,
        }).sync(events);
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
