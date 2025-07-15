import * as ADCSDK from '@api7/adc-sdk';
import axios, {
  type AxiosError,
  type AxiosInstance,
  type AxiosResponse,
} from 'axios';
import { produce } from 'immer';
import { curry, max, maxBy, unset } from 'lodash';
import {
  type ObservableInput,
  type Subject,
  catchError,
  from,
  iif,
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

const NEED_TO_INCREASE_CONF_VERSION = 'NEED_TO_INCREASE_CONF_VERSION';

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
    let newConfig: typing.APISIXStandaloneWithConfVersionType = oldConfig;

    const taskName = `Sync configuration`;
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(events).pipe(
      // derive the latest configuration from the old one
      // ensure no unexpected modifications through immutable objects
      tap((event) => {
        const resourceType =
          event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
            ? ADCSDK.ResourceType.CONSUMER
            : event.resourceType;
        const resourceKey = typing.APISIXStandaloneKeyMap[resourceType];
        const resourceIdKey =
          event.resourceType === ADCSDK.ResourceType.CONSUMER
            ? 'username'
            : 'id';
        const increaseVersionKey = `${resourceKey}.${NEED_TO_INCREASE_CONF_VERSION}`;
        if (event.type === ADCSDK.EventType.CREATE) {
          newConfig = produce(newConfig, (draft) => {
            if (!draft[resourceKey]) draft[resourceKey] = [];
            draft[resourceKey].push(
              this.fromADC({
                ...event,
                modifiedIndex: (draft[`${resourceKey}_conf_version`] ?? 0) + 1,
              }),
            );
            draft[increaseVersionKey] = true;
          });
        } else if (event.type === ADCSDK.EventType.UPDATE) {
          newConfig = produce(newConfig, (draft) => {
            const resources: Array<any> = draft[resourceKey];
            const index = resources.findIndex(
              (item) => item[resourceIdKey] === this.generateIdFromEvent(event),
            );
            if (index !== -1) {
              const eventResourceId = this.generateIdFromEvent(event);
              const newModifiedIndex = modifiedIndexMap.has(
                `${event.resourceType}.${eventResourceId}`,
              )
                ? (resources[index].modifiedIndex =
                    modifiedIndexMap.get(
                      `${event.resourceType}.${eventResourceId}`,
                    ) + 1)
                : 1;
              resources[index] = this.fromADC({
                ...event,
                modifiedIndex: newModifiedIndex,
              });
            }
            draft[increaseVersionKey] = true;
          });
        } else if (event.type === ADCSDK.EventType.DELETE) {
          newConfig = produce(newConfig, (draft) => {
            draft[resourceKey] = draft[resourceKey]?.filter(
              (item) => item[resourceIdKey] !== this.generateIdFromEvent(event),
            );
            draft[increaseVersionKey] = true;
          });
        }
      }),
      // filtering of new consumer configurations to ensure
      // that orphaned credential objects do not exist
      tap(() => {
        if (newConfig.consumers) {
          const consumers = newConfig.consumers
            .filter((item) => 'username' in item)
            .map((item) => item.username);

          newConfig = produce(newConfig, (draft) => {
            draft.consumers = draft.consumers.filter((consumer) => {
              if ('username' in consumer) return true;
              const credentialOnwer = consumer.id.split('/')?.[0];
              return consumers.includes(credentialOnwer); // filter orphan credentials
            });
          });
        }
      }),
      toArray(), // cumulative and combining events
      // update conf_version for each resource type
      tap(() => {
        const resourceTypes = Object.keys(typing.APISIXStandaloneKeyMap);
        resourceTypes.forEach((resourceType) => {
          newConfig = produce(newConfig, (draft) => {
            const resourceKey = typing.APISIXStandaloneKeyMap[resourceType];
            const confVersionKey = `${resourceKey}_conf_version`;
            const increaseVersionKey = `${resourceKey}.${NEED_TO_INCREASE_CONF_VERSION}`;
            const oldConfVersion = draft[confVersionKey];

            // Choose the larger of the old conf_version and the largest modifiedIndex to prevent
            // resource-level conf_version rewinds.
            draft[confVersionKey] = max([
              // do not set conf_version if it is already larger than the
              // maximum modifiedIndex of all resources
              draft[confVersionKey],
              // find the maximum modifiedIndex of all resources of this type
              maxBy<{ modifiedIndex: number }>(
                draft[typing.APISIXStandaloneKeyMap[resourceType]],
                'modifiedIndex',
              )?.modifiedIndex ?? 0,
            ]);

            // If the conf_version is not updated because the remote conf_version is too large and
            // exceeds the new modifiedIndex for any resource, a decision is made as to whether
            // the conf_version should be increased based on the flag used to indicate that it
            // should be updated.
            // Example:
            //   remote: {services_conf_version: 100, services: [{modifiedIndex: 1},{modifiedIndex: 2}]}
            //   local: {services: [{modifiedIndex: 1},{modifiedIndex: 3}]}
            //   Then the local will try to choose the larger of 100 and 3, and obviously 100 will be chosen
            //   That is, for the remote, conf_version is not updated, which is an exception,
            //   and in any case, conf_version needs to be greater than 100 in order to indicate that the
            //   cache is flushed.
            //   When a flag (NEED_INCREASE) exists and conf_version is not incremented, increment it manually.
            //   i.e., new local should be:
            //     {services_conf_version: 101, services: [{modifiedIndex: 1},{modifiedIndex: 3}]}
            if (
              draft[increaseVersionKey] &&
              oldConfVersion === draft[confVersionKey]
            )
              draft[confVersionKey] += 1;
            unset(draft, [increaseVersionKey]);
          });
        });
      }),
      switchMap(() =>
        iif(
          () => events.length > 0,
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
          of<ADCSDK.BackendSyncResult>({
            success: true,
            event: {} as ADCSDK.Event, // keep empty
            axiosResponse: null,
          } satisfies ADCSDK.BackendSyncResult),
        ),
      ),
    );
  }

  private generateIdFromEvent(event: ADCSDK.Event): string {
    if (event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL)
      return `${event.parentId}/credentials/${event.resourceId}`;
    return event.resourceId;
  }

  private extractModifiedIndex(oldConfig: typing.APISIXStandaloneType) {
    const modifiedIndexMap = new Map<string, number>();
    const extractModifiedIndex = curry(
      (prefix: string, item: { id?: string; modifiedIndex: number }) =>
        modifiedIndexMap.set(`${prefix}.${item.id}`, item.modifiedIndex),
    );
    oldConfig?.services?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.SERVICE),
    );
    oldConfig?.routes?.forEach(extractModifiedIndex(ADCSDK.ResourceType.ROUTE));
    oldConfig?.stream_routes?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.STREAM_ROUTE),
    );
    oldConfig?.upstreams?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.UPSTREAM),
    );
    oldConfig?.ssls?.forEach(extractModifiedIndex(ADCSDK.ResourceType.SSL));
    oldConfig?.consumers?.forEach((item) =>
      'username' in item
        ? extractModifiedIndex(ADCSDK.ResourceType.CONSUMER)({
            id: item.username,
            modifiedIndex: item.modifiedIndex,
          })
        : extractModifiedIndex(ADCSDK.ResourceType.CONSUMER_CREDENTIAL)(item),
    );
    oldConfig?.global_rules?.forEach(
      extractModifiedIndex(ADCSDK.ResourceType.GLOBAL_RULE),
    );
    oldConfig?.plugin_metadata?.forEach(
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
          id: this.generateIdFromEvent(event),
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
        const res = event.newValue as ADCSDK.Service;
        return {
          modifiedIndex: event.modifiedIndex,
          id: this.generateIdFromEvent(event),
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          hosts: res.hosts,
          upstream: this.fromADCUpstream(res.upstream),
          plugins: res.plugins,
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.CONSUMER: {
        type T = typing.APISIXStandaloneType['consumers'][number];
        const res = event.newValue as ADCSDK.Consumer;
        return {
          modifiedIndex: event.modifiedIndex,
          username: this.generateIdFromEvent(event),
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
          id: this.generateIdFromEvent(event),
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
          id: this.generateIdFromEvent(event),
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
          id: this.generateIdFromEvent(event),
          plugins: {
            [event.resourceId]: event.newValue as ADCSDK.GlobalRule,
          },
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.PLUGIN_METADATA: {
        type T = typing.APISIXStandaloneType['plugin_metadata'][number];
        return {
          modifiedIndex: event.modifiedIndex,
          id: this.generateIdFromEvent(event),
          ...event.newValue,
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.UPSTREAM: {
        type T = typing.APISIXStandaloneType['upstreams'][number];
        return {
          ...this.fromADCUpstream(
            event.newValue as ADCSDK.Upstream,
            event.parentId,
          ),
          modifiedIndex: event.modifiedIndex,
          id: this.generateIdFromEvent(event),
        } satisfies T as T;
      }
      case ADCSDK.ResourceType.STREAM_ROUTE: {
        type T = typing.APISIXStandaloneType['stream_routes'][number];
        const res = event.newValue as ADCSDK.StreamRoute;
        return {
          modifiedIndex: event.modifiedIndex,
          id: this.generateIdFromEvent(event),
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

  private fromADCUpstream(
    res: ADCSDK.Upstream,
    parentId?: string,
  ): typing.APISIXStandaloneType['upstreams'][number] {
    type T = ReturnType<typeof this.fromADCUpstream>;
    const upstream = {
      modifiedIndex: undefined, // fill in later
      id: undefined, // fill in later
      name: res.name,
      desc: res.description,
      labels: this.fromADCLabels(res.labels),
      type: res.type,
      hash_on: res.hash_on,
      key: res.key,
      nodes: res.nodes ?? [], // fix optional to required convert
      scheme: res.scheme,
      retries: res.retries,
      retry_timeout: res.retry_timeout,
      timeout: res.timeout,
      tls: res.tls,
      keepalive_pool: res.keepalive_pool,
      pass_host: res.pass_host,
      upstream_host: res.upstream_host,
    } satisfies T as T;
    if (parentId)
      upstream.labels = {
        ...upstream.labels,
        [typing.ADC_UPSTREAM_SERVICE_ID_LABEL]: parentId,
      };
    return upstream;
  }
}
