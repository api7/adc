import * as ADCSDK from '@api7/adc-sdk';
import axios, {
  type AxiosError,
  type AxiosInstance,
  type AxiosResponse,
} from 'axios';
import { produce } from 'immer';
import { curry, maxBy } from 'lodash';
import {
  type ObservableInput,
  type Subject,
  catchError,
  from,
  map,
  of,
  switchMap,
  tap,
  throwError,
  toArray,
} from 'rxjs';
import { type SemVer } from 'semver';

import * as typing from './typing';

type EventWithModifiedIndex = ADCSDK.Event & {
  modifiedIndex: number;
};

export interface OperatorOptions {
  client: AxiosInstance;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
}
export class Operator extends ADCSDK.backend.BackendEventSource {
  private readonly client: AxiosInstance;

  constructor(private readonly opts: OperatorOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  public sync(
    events: Array<ADCSDK.Event>,
    oldConfig: typing.APISIXStandaloneWithConfVersionType,
    opts: ADCSDK.BackendSyncOptions = { exitOnFailure: true },
  ) {
    const modifiedIndexMap = this.extractModifiedIndex(oldConfig);
    let newConfig: typing.APISIXStandaloneWithConfVersionType =
      structuredClone(oldConfig);

    const taskName = `Sync configuration`;
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(events).pipe(
      // derive the latest configuration from the old one
      // ensure no unexpected modifications through immutable objects
      tap((event) => {
        if (event.type === ADCSDK.EventType.CREATE) {
          newConfig = produce(newConfig, (draft) => {
            if (!draft[typing.APISIXStandaloneKeyMap[event.resourceType]])
              draft[typing.APISIXStandaloneKeyMap[event.resourceType]] = [];
            draft[typing.APISIXStandaloneKeyMap[event.resourceType]].push(
              this.fromADC({ ...event, modifiedIndex: 1 }),
            );
          });
        } else if (event.type === ADCSDK.EventType.UPDATE) {
          newConfig = produce(newConfig, (draft) => {
            const resources: Array<any> =
              draft[typing.APISIXStandaloneKeyMap[event.resourceType]];
            const index = resources.findIndex(
              (item) => item.id === event.resourceId, //TODO: handle parentId
            );
            if (index !== -1) {
              const newModifiedIndex = modifiedIndexMap.has(
                `${event.resourceType}.${event.resourceId}`,
              )
                ? (resources[index].modifiedIndex =
                    modifiedIndexMap.get(
                      `${event.resourceType}.${event.resourceId}`,
                    ) + 1)
                : 1;
              resources[index] = this.fromADC({
                ...event,
                modifiedIndex: newModifiedIndex,
              });
            }
          });
        } else if (event.type === ADCSDK.EventType.DELETE) {
          newConfig = produce(newConfig, (draft) => {
            draft[typing.APISIXStandaloneKeyMap[event.resourceType]] = draft[
              typing.APISIXStandaloneKeyMap[event.resourceType]
            ].filter((item) => item.id !== event.resourceId);
          });
        }
      }),
      toArray(), // cumulative and combining events
      // update conf_version for each resource type
      tap(() => {
        const resourceTypes = Object.keys(typing.APISIXStandaloneKeyMap);
        resourceTypes.forEach((resourceType) => {
          newConfig = produce(newConfig, (draft) => {
            draft[
              `${typing.APISIXStandaloneKeyMap[resourceType]}_conf_version`
            ] =
              maxBy<{ modifiedIndex: number }>(
                draft[typing.APISIXStandaloneKeyMap[resourceType]],
                'modifiedIndex',
              )?.modifiedIndex ?? 0;
          });
        });
      }),
      switchMap(() =>
        from(this.client.put('/apisix/admin/configs', newConfig)).pipe(
          tap((resp) => logger(this.debugLogEvent(resp))),
          map<AxiosResponse, ADCSDK.BackendSyncResult>(
            (response) =>
              ({
                success: true,
                event: {} as ADCSDK.Event, // keep empty
                axiosResponse: response,
              }) satisfies ADCSDK.BackendSyncResult,
          ),
          catchError<
            ADCSDK.BackendSyncResult,
            ObservableInput<ADCSDK.BackendSyncResult>
          >((error: Error | AxiosError) => {
            if (opts.exitOnFailure) {
              if (axios.isAxiosError(error) && error.response)
                return throwError(
                  () =>
                    new Error(
                      error.response?.data?.error_msg ??
                        JSON.stringify(error.response?.data),
                    ),
                );
              return throwError(() => error);
            }
            return of({
              success: false,
              event: {} as ADCSDK.Event, // keep empty,
              error,
              ...(axios.isAxiosError(error) && {
                axiosResponse: error.response,
                ...(error.response?.data?.error_msg && {
                  error: new Error(error.response.data.error_msg),
                }),
              }),
            } satisfies ADCSDK.BackendSyncResult);
          }),
          tap(() => logger(taskStateEvent('TASK_DONE'))),
        ),
      ),
    );
  }

  private extractModifiedIndex(oldConfig: typing.APISIXStandaloneType) {
    const modifiedIndexMap = new Map<string, number>();
    const extractModifiedIndex = curry(
      (prefix: string, item: { id: string; modifiedIndex: number }) =>
        modifiedIndexMap.set(`${prefix}.${item.id}`, item.modifiedIndex),
    );
    oldConfig.services?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.SERVICE),
    );
    oldConfig.routes?.forEach(extractModifiedIndex(ADCSDK.ResourceType.ROUTE));
    oldConfig.stream_routes?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.STREAM_ROUTE),
    );
    oldConfig.upstreams?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.UPSTREAM),
    );
    oldConfig.ssls?.forEach(extractModifiedIndex(ADCSDK.ResourceType.SSL));
    oldConfig.consumers?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.CONSUMER),
    );
    oldConfig.global_rules?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.GLOBAL_RULE),
    );
    oldConfig.plugin_metadata?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.PLUGIN_METADATA),
    );
    return modifiedIndexMap;
  }

  private fromADC(event: EventWithModifiedIndex) {
    switch (event.resourceType) {
      case ADCSDK.ResourceType.ROUTE: {
        type T = typing.APISIXStandaloneType['routes'][number];
        const res = event.newValue as ADCSDK.Route;
        return {
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          uris: res.uris,
          hosts: res.hosts,
          methods: res.methods,
          remote_addrs: res.remote_addrs,
          vars: res.vars,
          filter_func: res.filter_func,
          service_id: event.parentId,
          enable_websocket: res.enable_websocket,
          plugins: res.plugins,
          priority: res.priority,
          timeout: res.timeout,
          status: 1,
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.SERVICE: {
        type T = typing.APISIXStandaloneType['services'][number];
        type TU = typing.APISIXStandaloneType['upstreams'][number];
        const res = event.newValue as ADCSDK.Service;
        return {
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          hosts: res.hosts,
          upstream: res.upstream as TU,
          plugins: res.plugins,
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.CONSUMER: {
        type T = typing.APISIXStandaloneType['consumers'][number];
        const res = event.newValue as ADCSDK.Consumer;
        return {
          modifiedIndex: event.modifiedIndex,
          username: event.resourceId,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          plugins: res.plugins,
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.CONSUMER_CREDENTIAL: {
        type T = typing.APISIXStandaloneType['consumers'][number];
        const res = event.newValue as ADCSDK.ConsumerCredential;
        return {
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          plugins: {
            [res.type]: res.config,
          },
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.SSL: {
        type T = typing.APISIXStandaloneType['ssls'][number];
        const res = event.newValue as ADCSDK.SSL;
        return {
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          labels: this.fromADCLabels(res.labels),
          type: res.type,
          snis: res.snis,
          cert: res.certificates[0].certificate,
          key: res.certificates[0].key,
          ...(res.certificates.length > 1
            ? {
                certs: res.certificates
                  .slice(1)
                  .map((cert) => cert.certificate),
                keys: res.certificates.slice(1).map((cert) => cert.key),
              }
            : {}),
          client: res.client,
          ssl_protocols: res.ssl_protocols,
          status: 1,
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.GLOBAL_RULE: {
        type T = typing.APISIXStandaloneType['global_rules'][number];
        return {
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          plugins: {
            [event.resourceId]: event.newValue as ADCSDK.GlobalRule,
          },
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.PLUGIN_METADATA: {
        type T = typing.APISIXStandaloneType['plugin_metadata'][number];
        return {
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          ...event.newValue,
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.UPSTREAM: {
        type T = typing.APISIXStandaloneType['upstreams'][number];
        const res = event.newValue as ADCSDK.Upstream;
        const upstream = {
          ...res,
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          nodes: res.nodes ?? [], // fix optional to required convert
        } satisfies T as T;
        if (event.parentId)
          upstream.labels = {
            ...upstream.labels,
            [typing.ADC_UPSTREAM_SERVICE_ID_LABEL]: event.parentId,
          };
        return upstream;
      }
      case ADCSDK.ResourceType.STREAM_ROUTE: {
        type T = typing.APISIXStandaloneType['stream_routes'][number];
        const res = event.newValue as ADCSDK.StreamRoute;
        return {
          modifiedIndex: event.modifiedIndex,
          id: event.resourceId,
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          plugins: res.plugins,
          remote_addr: res.remote_addr,
          server_addr: res.server_addr,
          server_port: res.server_port,
          sni: res.sni,
          service_id: event.parentId,
        } satisfies T as T;
      }
    }
  }

  private fromADCLabels(labels?: ADCSDK.Labels): Record<string, string> {
    if (!labels) return undefined;
    return Object.entries(labels).reduce((pv, [key, value]) => {
      pv[key] = typeof value === 'string' ? value : JSON.stringify(value);
      return pv;
    }, {});
  }
}
