import * as ADCSDK from '@api7/adc-sdk';
import axios, {
  Axios,
  AxiosError,
  AxiosResponse,
  CreateAxiosDefaults,
} from 'axios';
import { JSONSchema4 } from 'json-schema';
import { Listr, ListrTask } from 'listr2';
import { isEmpty, isNil } from 'lodash';
import EventEmitter from 'node:events';
import { readFileSync } from 'node:fs';
import {
  Agent as httpAgent,
  AgentOptions as httpAgentOptions,
} from 'node:http';
import {
  Agent as httpsAgent,
  AgentOptions as httpsAgentOptions,
} from 'node:https';
import {
  Observable,
  ObservableInput,
  catchError,
  concatMap,
  filter,
  finalize,
  forkJoin,
  from,
  map,
  mergeMap,
  of,
  reduce,
  switchMap,
  tap,
  toArray,
} from 'rxjs';
import semver, { SemVer } from 'semver';

import { Fetcher } from './fetcher';
import { Operator } from './operator';
import { ToADC } from './transformer';
import { capitalizeFirstLetter } from './utils';

export class BackendAPI7 implements ADCSDK.Backend {
  private readonly client: Axios;
  private readonly logger: ADCSDK.Logger;
  private readonly gatewayGroupName: string;
  private static logScope = ['API7'];

  private version?: SemVer;
  private gatewayGroupId?: string;
  private innerDefaultValue: ADCSDK.DefaultValue;

  private readonly emitter = new EventEmitter();

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
    this.logger = opts.logger;
  }

  public async ping() {
    await this.client.get('/api/gateway_groups');
  }

  public async defaultValue() {
    if (this.defaultValue) return this.innerDefaultValue;
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
    this.emitter.emit(ADCSDK.BackendEventType.AXIOS_DEBUG, {
      response: resp,
      description: 'Get core resoruces schema',
    } satisfies ADCSDK.BackendEventAxiosDebug);

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
    this.emitter.emit(ADCSDK.BackendEventType.AXIOS_DEBUG, {
      response: resp,
      description: 'Get core resoruces schema',
    } satisfies ADCSDK.BackendEventAxiosDebug);

    const gatewayGroups = resp?.data?.list;
    if (!gatewayGroups?.length)
      throw Error(`Gateway group "${this.gatewayGroupName}" does not exist`);
    return (this.gatewayGroupId = gatewayGroups[0].id);
  }

  private async getVersion() {
    if (this.version) return this.version;

    const resp = await this.client.get<{ value: string }>('/api/version');
    this.emitter.emit(ADCSDK.BackendEventType.AXIOS_DEBUG, {
      response: resp,
      description: `Get API7 version`,
    } satisfies ADCSDK.BackendEventAxiosDebug);

    return (this.version =
      resp?.data?.value === 'dev'
        ? semver.coerce('999.999.999')
        : semver.coerce(resp?.data?.value) || semver.coerce('0.0.0'));
  }

  public async dump0(): Promise<Listr<{ remote: ADCSDK.Configuration }>> {
    return new Listr<{ remote: ADCSDK.Configuration }>(
      [
        {
          task: (ctx) => {
            ctx.remote = {};
          },
        },
      ],
      {
        //@ts-expect-error reorg renderer
        rendererOptions: { scope: BackendAPI7.logScope },
      },
    );
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
          eventEmitter: this.emitter,
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
    const generateTaskName = (event: ADCSDK.Event) => {
      return `${capitalizeFirstLetter(
        event.type,
      )} ${event.resourceType}: "${event.resourceName}"`;
    };

    return forkJoin([
      from(this.getVersion()),
      from(this.getGatewayGroupId()),
    ]).pipe(
      switchMap(([version, gatewayGroupId]) => {
        const operator = new Operator({
          client: this.client,
          version,
          eventEmitter: this.emitter,
          gatewayGroupId,
          gatewayGroupName: this.gatewayGroupName,
        });
        return this.syncPreprocessEvents(events).pipe<ADCSDK.BackendSyncResult>(
          concatMap((group) =>
            from(group).pipe(
              mergeMap((event) =>
                from(
                  event.type === ADCSDK.EventType.DELETE
                    ? operator.deleteResource(event)
                    : operator.updateResource(event),
                ).pipe(
                  tap((resp) => {
                    this.emitter.emit(ADCSDK.BackendEventType.TASK_START, {
                      name: generateTaskName(event),
                    } satisfies ADCSDK.BackendEventTaskState);
                    /* this.emitter.emit(ADCSDK.BackendEventType.AXIOS_DEBUG, {
                      resp,
                    }); */
                  }),
                  map<AxiosResponse, ADCSDK.BackendSyncResult>((response) => {
                    return {
                      success: true,
                      event,
                      axiosResponse: response,
                    } satisfies ADCSDK.BackendSyncResult;
                  }),
                  catchError<
                    ADCSDK.BackendSyncResult,
                    ObservableInput<ADCSDK.BackendSyncResult>
                  >((error: Error | AxiosError) => {
                    //TODO error handler
                    if (axios.isAxiosError(error)) {
                      if (error.response) {
                        //TODO 有响应的状态码失败
                      } else {
                        //TODO 无正常响应
                      }
                      if (opts.exitOnFailed) throw error;
                      return of({
                        success: false,
                        event,
                        axiosResponse: error.response,
                        error,
                      } satisfies ADCSDK.BackendSyncResult);
                    }
                  }),
                  finalize(() =>
                    this.emitter.emit(ADCSDK.BackendEventType.TASK_DONE, {
                      name: generateTaskName(event),
                    } satisfies ADCSDK.BackendEventTaskState),
                  ),
                ),
              ),
            ),
          ),
        );
      }),
    );
  }

  // Preprocess events for sync:
  // 1. Events that attempt to remove routes but not for the purpose of
  //    updating the service will be ignored.
  // 2. The service will at least be removed from the gateway group, i.e.,
  //    it will stop processing such traffic.
  // 3. Divide events into groups by resource type and operation type.
  private syncPreprocessEvents(events: Array<ADCSDK.Event>) {
    const isRouteLike = (event: ADCSDK.Event) =>
      [ADCSDK.ResourceType.ROUTE, ADCSDK.ResourceType.STREAM_ROUTE].includes(
        event.resourceType,
      );

    const event$ = from(events);
    return event$.pipe(
      // Aggregate services that need to be deleted
      filter(
        (event) =>
          event.resourceType === ADCSDK.ResourceType.SERVICE &&
          event.type === ADCSDK.EventType.DELETE,
      ),
      map((event) => event.resourceId),
      toArray(),
      // Switch to a new event pipe for event filtering and grouping.
      // It will use the deleted service ID that has been aggregated.
      switchMap((deletedServiceIds) =>
        event$.pipe(
          // If an event wants to delete a route, but its parent service
          // will also be deleted, this operation can be ignored.
          // The deletion service will cascade the deletion of the route.
          filter(
            (event) =>
              !(
                isRouteLike(event) &&
                event.type === ADCSDK.EventType.DELETE &&
                deletedServiceIds.includes(event.parentId)
              ),
          ),
          // Grouping events by resource type and operation type.
          // The sequence of events should not be broken in this process,
          // and the correct behavior of the API will depend on the order
          // of execution.
          reduce((groups, event) => {
            const key = `${event.resourceType}.${event.type}`;
            (groups[key] = groups[key] || []).push(event);
            return groups;
          }, {}),
          // Strip group name and convert to two-dims arrays
          // {"service.create": [1], "consumer.create": [2]} => [[1], [2]]
          mergeMap<
            Record<string, Array<ADCSDK.Event>>,
            Observable<Array<ADCSDK.Event>>
          >((obj) => from(Object.values(obj))),
        ),
      ),
    );
  }

  public on(
    eventType: keyof typeof ADCSDK.BackendEventType,
    cb: (...args: any[]) => void,
  ) {
    this.emitter.on(eventType, cb);
  }
  public removeAllListeners() {
    this.emitter.removeAllListeners();
  }
}
