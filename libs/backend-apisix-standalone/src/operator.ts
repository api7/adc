import * as ADCSDK from '@api7/adc-sdk';
import axios, {
  type AxiosError,
  type AxiosInstance,
  type AxiosResponse,
} from 'axios';
import { cloneDeep, unset } from 'lodash';
import { createHash } from 'node:crypto';
import {
  type ObservableInput,
  type Subject,
  catchError,
  from,
  map,
  mergeMap,
  of,
  switchMap,
  tap,
  throwError,
  toArray,
} from 'rxjs';
import { type SemVer } from 'semver';

import { config as configCache, rawConfig as rawConfigCache } from './cache';
import { ENDPOINT_CONFIG, HEADER_CREDENTIAL, HEADER_DIGEST } from './constants';
import { toADC } from './transformer';
import * as typing from './typing';

export interface OperatorOptions {
  cacheKey: string;
  client: AxiosInstance;
  serverTokenMap: typing.ServerTokenMap;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  oldRawConfiguration: typing.APISIXStandalone;
}

export class Operator extends ADCSDK.backend.BackendEventSource {
  constructor(private readonly opts: OperatorOptions) {
    super();
    this.subject = opts.eventSubject;
  }

  public sync(
    events: Array<ADCSDK.Event>,
    opts: ADCSDK.BackendSyncOptions = { exitOnFailure: true },
  ) {
    const newConfig = cloneDeep<typing.APISIXStandalone>(
      this.opts.oldRawConfiguration,
    );
    unset(newConfig, 'X-Last-Modified');
    unset(newConfig, 'X-Digest');
    const increaseVersion: Partial<Record<typing.UsedResourceTypes, boolean>> =
      {};

    const taskName = `Sync configuration`;
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));

    const timestamp = Date.now();
    return from(events).pipe(
      // derive the latest configuration from the old config
      tap((event) =>
        this.applyEvent(newConfig, increaseVersion, timestamp, event),
      ),
      // filtering of new consumer configurations to ensure
      // that orphaned credential objects do not exist
      tap(() => {
        if (newConfig.consumers) {
          const consumers = newConfig.consumers
            .filter((item) => 'username' in item)
            .map((item) => item.username);

          newConfig.consumers = newConfig?.consumers?.filter((consumer) => {
            if ('username' in consumer) return true;
            const credentialOwner = consumer.id.split('/')?.[0];
            return consumers.includes(credentialOwner); // filter orphan credentials
          });
        }
      }),
      toArray(), // cumulative and combining events
      // update conf_version for each resource type
      tap(() => {
        const resourceTypes = Object.keys(
          typing.APISIXStandaloneKeyMap,
        ) as unknown as Array<typing.UsedResourceTypes>;
        resourceTypes
          .filter((item) => increaseVersion[item])
          .forEach((resourceType) => {
            newConfig[
              `${typing.APISIXStandaloneKeyMap[resourceType]}_conf_version`
            ] = timestamp;
          });
      }),
      switchMap(() =>
        from(this.opts.serverTokenMap).pipe(
          mergeMap(([server, token]) =>
            from(
              this.opts.client.put(`${server}${ENDPOINT_CONFIG}`, newConfig, {
                headers: {
                  [HEADER_CREDENTIAL]: token,
                  [HEADER_DIGEST]: createHash('sha1')
                    .update(JSON.stringify(newConfig))
                    .digest('hex'),
                },
              }),
            ).pipe(
              tap((resp) => logger(this.debugLogEvent(resp))),
              map<AxiosResponse, ADCSDK.BackendSyncResult>(
                (response) =>
                  ({
                    success: true,
                    event: {} as ADCSDK.Event, // keep empty
                    axiosResponse: response,
                    server,
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
                  server,
                } satisfies ADCSDK.BackendSyncResult);
              }),
              tap(() => {
                configCache.set(this.opts.cacheKey, toADC(newConfig));
                rawConfigCache.set(this.opts.cacheKey, newConfig);
                logger(taskStateEvent('TASK_DONE'));
              }),
            ),
          ),
        ),
      ),
    );
  }

  private applyEvent(
    config: typing.APISIXStandalone,
    increaseVersion: Partial<Record<typing.UsedResourceTypes, boolean>>,
    timestamp: number,
    event: ADCSDK.Event,
  ) {
    const resourceType =
      event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL
        ? ADCSDK.ResourceType.CONSUMER
        : (event.resourceType as typing.UsedResourceTypes);
    const resourceKey = typing.APISIXStandaloneKeyMap[resourceType];

    if (event.resourceType === ADCSDK.ResourceType.SERVICE)
      this.applyEventForServiceInlinedUpstream(
        config,
        increaseVersion,
        timestamp,
        event,
      );

    if (event.type === ADCSDK.EventType.CREATE) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any -- infer error
      (config[resourceKey] ||= []).push(this.fromADC(event, timestamp) as any);
      increaseVersion[resourceType] = true;
    } else if (event.type === ADCSDK.EventType.UPDATE) {
      // Only update the service when the service itself is modified, to avoid service
      // conf version (modifiedIndex) changes caused by changes to its inline upstream.
      if (
        event.resourceType === ADCSDK.ResourceType.SERVICE &&
        (event.diff || []).filter((item) => item.path?.[0] !== 'upstream')
          .length <= 0
      )
        return;

      config[resourceKey] ||= [];
      const resources = config[resourceKey];
      const index = resources.findIndex(
        (item) =>
          ('id' in item ? item.id : item.username) ===
          this.generateIdFromEvent(event),
      );
      if (index !== -1) {
        resources[index] = this.fromADC(
          event,
          timestamp,
        ) as (typeof resources)[number];
        increaseVersion[resourceType] = true;
      }
    } else {
      // If the resource does not exist, there is no need to delete it.
      if (!config[resourceKey]) return;

      const resources = config[resourceKey];
      const index = resources.findIndex(
        (item) =>
          ('id' in item ? item.id : item.username) ===
          this.generateIdFromEvent(event),
      );
      if (index !== -1) {
        resources.splice(index, 1);
        increaseVersion[resourceType] = true;
      }
    }
  }

  private applyEventForServiceInlinedUpstream(
    config: typing.APISIXStandalone,
    increaseVersion: Partial<Record<typing.UsedResourceTypes, boolean>>,
    timestamp: number,
    event: ADCSDK.Event,
  ) {
    if (event.resourceType !== ADCSDK.ResourceType.SERVICE) return;

    const upstreamResourceKey =
      typing.APISIXStandaloneKeyMap[ADCSDK.ResourceType.UPSTREAM];

    if (event.type === ADCSDK.EventType.CREATE) {
      (config[upstreamResourceKey] ||= []).push({
        ...this.fromADCUpstream(
          (event.newValue as ADCSDK.Service).upstream as ADCSDK.Upstream,
        ),
        id: event.resourceId,
        modifiedIndex: timestamp,
        name: event.resourceName,
      });
      increaseVersion[ADCSDK.ResourceType.UPSTREAM] = true;
    } else if (event.type === ADCSDK.EventType.UPDATE) {
      if (
        (event.diff || []).filter((item) => item.path?.[0] === 'upstream')
          .length <= 0
      )
        return;

      config[upstreamResourceKey] ||= [];
      const resources = config[upstreamResourceKey];
      const index = resources.findIndex((item) => item.id === event.resourceId);
      if (index != -1) {
        resources[index] = {
          ...this.fromADCUpstream(
            (event.newValue as ADCSDK.Service).upstream as ADCSDK.Upstream,
          ),
          id: event.resourceId,
          modifiedIndex: timestamp,
          name: event.resourceName,
        };
        increaseVersion[ADCSDK.ResourceType.UPSTREAM] = true;
      }
    } else if (event.type === ADCSDK.EventType.DELETE) {
      config[upstreamResourceKey] ||= [];
      const resources = config[upstreamResourceKey];
      const index = resources.findIndex((item) => item.id === event.resourceId);
      if (index != -1) {
        resources.splice(index, 1);
        increaseVersion[ADCSDK.ResourceType.UPSTREAM] = true;
      }
    }
  }

  private generateIdFromEvent(event: ADCSDK.Event): string {
    if (event.resourceType === ADCSDK.ResourceType.CONSUMER_CREDENTIAL)
      return `${event.parentId}/credentials/${event.resourceId}`;
    return event.resourceId;
  }

  private fromADC(event: ADCSDK.Event, modifiedIndex: number) {
    switch (
      event.resourceType as
        | typing.UsedResourceTypes
        | ADCSDK.ResourceType.CONSUMER_CREDENTIAL
    ) {
      case ADCSDK.ResourceType.ROUTE: {
        const res = event.newValue as ADCSDK.Route;
        return {
          modifiedIndex,
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
          service_id: event.parentId!,
          enable_websocket: res.enable_websocket,
          plugins: res.plugins,
          priority: res.priority,
          timeout: res.timeout,
          status: 1,
        } satisfies typing.Route as typing.Route;
      }
      case ADCSDK.ResourceType.SERVICE: {
        const res = event.newValue as ADCSDK.Service;
        const id = this.generateIdFromEvent(event);
        return {
          modifiedIndex,
          id,
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          hosts: res.hosts,
          upstream_id: id,
          plugins: res.plugins,
        } satisfies typing.Service as typing.Service;
      }
      case ADCSDK.ResourceType.CONSUMER: {
        const res = event.newValue as ADCSDK.Consumer;
        return {
          modifiedIndex,
          username: this.generateIdFromEvent(event),
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          plugins: res.plugins,
        } satisfies typing.Consumer as typing.Consumer;
      }
      case ADCSDK.ResourceType.CONSUMER_CREDENTIAL: {
        const res = event.newValue as ADCSDK.ConsumerCredential;
        return {
          modifiedIndex,
          id: this.generateIdFromEvent(event),
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          plugins: {
            [res.type]: res.config,
          },
        } satisfies typing.ConsumerCredential as typing.ConsumerCredential;
      }
      case ADCSDK.ResourceType.SSL: {
        const res = event.newValue as ADCSDK.SSL;
        return {
          modifiedIndex,
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
        } satisfies typing.SSL as typing.SSL;
      }
      case ADCSDK.ResourceType.GLOBAL_RULE: {
        return {
          modifiedIndex,
          id: this.generateIdFromEvent(event),
          plugins: {
            [event.resourceId]: event.newValue as ADCSDK.GlobalRule,
          },
        } satisfies typing.GlobalRule as typing.GlobalRule;
      }
      case ADCSDK.ResourceType.PLUGIN_METADATA: {
        return {
          modifiedIndex,
          id: this.generateIdFromEvent(event),
          ...event.newValue,
        } satisfies typing.PluginMetadata as typing.PluginMetadata;
      }
      case ADCSDK.ResourceType.UPSTREAM: {
        return {
          ...this.fromADCUpstream(
            event.newValue as ADCSDK.Upstream,
            event.parentId,
          ),
          modifiedIndex,
          id: this.generateIdFromEvent(event),
        } satisfies typing.Upstream as typing.Upstream;
      }
      case ADCSDK.ResourceType.STREAM_ROUTE: {
        const res = event.newValue as ADCSDK.StreamRoute;
        return {
          modifiedIndex,
          id: this.generateIdFromEvent(event),
          name: res.name,
          desc: res.description,
          labels: this.fromADCLabels(res.labels),
          plugins: res.plugins,
          remote_addr: res.remote_addr,
          server_addr: res.server_addr,
          server_port: res.server_port,
          sni: res.sni,
          service_id: event.parentId!,
        } satisfies typing.StreamRoute as typing.StreamRoute;
      }
    }
  }

  private fromADCLabels(
    labels?: ADCSDK.Labels,
  ): Record<string, string> | undefined {
    if (!labels) return undefined;
    return Object.entries(labels).reduce(
      (pv, [key, value]) => {
        pv[key] = typeof value === 'string' ? value : JSON.stringify(value);
        return pv;
      },
      {} as Record<string, string>,
    );
  }

  private fromADCUpstream(
    res: ADCSDK.Upstream,
    parentId?: string,
  ): typing.Upstream {
    const upstream = {
      modifiedIndex: undefined!,
      id: undefined!, // fill in later
      name: res.name!,
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
      checks: res.checks,
      discovery_type: res.discovery_type,
      service_name: res.service_name,
      discovery_args: res.discovery_args,
    } satisfies typing.Upstream as typing.Upstream;
    if (parentId)
      upstream.labels = {
        ...upstream.labels,
        [typing.ADC_UPSTREAM_SERVICE_ID_LABEL]: parentId,
      };
    return upstream;
  }
}
