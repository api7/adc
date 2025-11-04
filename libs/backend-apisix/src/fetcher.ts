import * as ADCSDK from '@api7/adc-sdk';
import { type AxiosInstance } from 'axios';
import { produce } from 'immer';
import { unset } from 'lodash';
import {
  Subject,
  combineLatest,
  finalize,
  from,
  iif,
  map,
  mergeMap,
  of,
  switchMap,
  takeLast,
  tap,
  toArray,
} from 'rxjs';
import { SemVer, gte as semVerGTE } from 'semver';

import { ToADC } from './transformer';
import * as typing from './typing';
import { resourceTypeToAPIName } from './utils';

export interface FetcherOptions {
  client: AxiosInstance;
  version: SemVer;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  backendOpts: ADCSDK.BackendOptions;
}
export class Fetcher extends ADCSDK.backend.BackendEventSource {
  private readonly client: AxiosInstance;

  private readonly toADC = new ToADC();

  constructor(private readonly opts: FetcherOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  private _list<RESP_TYPE>(resourceType: ADCSDK.ResourceType) {
    const apiName = resourceTypeToAPIName(resourceType);
    const taskName = `Fetch ${apiName}`;
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(this.client.get<RESP_TYPE>(`/apisix/admin/${apiName}`)).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      map((resp) => resp.data),
      finalize(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  private listServices() {
    return this._list<typing.ListResponse<typing.Service>>(
      ADCSDK.ResourceType.SERVICE,
    ).pipe(map((item) => item.list.map((item) => item.value)));
  }

  private listRoutes() {
    return this._list<typing.ListResponse<typing.Route>>(
      ADCSDK.ResourceType.ROUTE,
    ).pipe(map((item) => item.list.map((item) => item.value)));
  }

  private listUpstreams() {
    return this._list<typing.ListResponse<typing.Upstream>>(
      ADCSDK.ResourceType.UPSTREAM,
    ).pipe(map((item) => item.list.map((item) => item.value)));
  }

  private listSSLs() {
    return this._list<typing.ListResponse<typing.SSL>>(
      ADCSDK.ResourceType.SSL,
    ).pipe(map((item) => item.list.map((item) => item.value)));
  }

  private listConsumers() {
    const taskName = 'Fetch consumers';
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<typing.ListResponse<typing.Consumer>>(
        `/apisix/admin/consumers`,
      ),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      mergeMap((resp) => {
        const obs = from(resp.data.list);
        return iif(
          () => semVerGTE(this.opts.version, '3.11.0'),
          obs.pipe(
            mergeMap(({ value: consumer }) =>
              from(
                this.client.get<typing.ListResponse<typing.ConsumerCredential>>(
                  `/apisix/admin/consumers/${consumer.username}/credentials`,
                  { validateStatus: () => true },
                ),
              ).pipe(
                tap((resp) =>
                  logger(
                    this.debugLogEvent(
                      resp,
                      `Get credentials of consumer "${consumer.username}"`,
                    ),
                  ),
                ),
                map((resp) => {
                  if (resp.status === 404) return consumer;
                  return produce(consumer, (draft) => {
                    draft.credentials = resp?.data?.list?.map(
                      (item) => item.value,
                    );
                  });
                }),
              ),
            ),
          ),
          obs.pipe(map((item) => item.value)),
        );
      }),
      toArray(),
      tap(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  private listPluginConfigs() {
    return this._list<typing.ListResponse<typing.PluginConfig>>(
      ADCSDK.ResourceType.PLUGIN_CONFIG,
    ).pipe(map((item) => item.list.map((item) => item.value)));
  }

  private listGlobalRules() {
    return this._list<typing.ListResponse<Record<string, typing.GlobalRule>>>(
      ADCSDK.ResourceType.GLOBAL_RULE,
    ).pipe<ADCSDK.Plugins>(
      map(({ list }) =>
        // [{ 'key-auth': {}, 'basic-auth': {} }, { 'real-ip': {} }] =>
        //  { 'key-auth': {}, 'basic-auth': {}, 'real-ip': {} }
        Object.fromEntries(
          list.flatMap((item) => Object.entries(item.value?.plugins ?? [])),
        ),
      ),
    );
  }
  private listPluginMetadata() {
    return this._list<
      typing.ListResponse<Record<string, typing.PluginMetadata>>
    >(ADCSDK.ResourceType.PLUGIN_METADATA).pipe<ADCSDK.Plugins>(
      map(({ list }) =>
        Object.fromEntries<ADCSDK.Plugin>(
          list.map((item) => [
            item.key.split('/').pop() ?? item.key,
            ADCSDK.utils.recursiveOmitUndefined(item.value),
          ]),
        ),
      ),
    );
  }

  private listStreamRoute() {
    const taskName = 'Fetch stream_routes';
    const logger = this.getLogger(taskName);
    const taskStateEvent = this.taskStateEvent(taskName);
    logger(taskStateEvent('TASK_START'));
    return from(
      this.client.get<typing.ListResponse<typing.StreamRoute>>(
        `/apisix/admin/stream_routes`,
        { validateStatus: () => true },
      ),
    ).pipe(
      tap((resp) => logger(this.debugLogEvent(resp))),
      map((resp) => {
        if (resp.status >= 400) return [] as Array<typing.StreamRoute>;
        return resp.data?.list?.map((item) => item.value);
      }),
      finalize(() => logger(taskStateEvent('TASK_DONE'))),
    );
  }

  public dump() {
    return combineLatest([
      this.listServices(),
      this.listRoutes(),
      this.listUpstreams(),
      this.listSSLs(),
      this.listConsumers(),
      this.listPluginConfigs(),
      this.listGlobalRules(),
      this.listPluginMetadata(),
      this.listStreamRoute(),
    ]).pipe(
      takeLast(1),
      map(
        ([
          services,
          routes,
          upstreams,
          ssls,
          consumers,
          plugin_configs,
          global_rules,
          plugin_metadata,
          stream_routes,
        ]) =>
          ({
            [ADCSDK.ResourceType.SERVICE]: services,
            [ADCSDK.ResourceType.ROUTE]: routes,
            [ADCSDK.ResourceType.UPSTREAM]: upstreams,
            [ADCSDK.ResourceType.SSL]: ssls,
            [ADCSDK.ResourceType.CONSUMER]: consumers,
            [ADCSDK.ResourceType.PLUGIN_CONFIG]: plugin_configs,
            [ADCSDK.ResourceType.GLOBAL_RULE]: global_rules,
            [ADCSDK.ResourceType.PLUGIN_METADATA]: plugin_metadata,
            [ADCSDK.ResourceType.STREAM_ROUTE]: stream_routes,
          }) satisfies typing.Resources as typing.Resources,
      ),
      // Move plugin templates to route
      map((resources) => {
        const pluginConfigIdMap = Object.fromEntries(
          (resources?.[ADCSDK.ResourceType.PLUGIN_CONFIG] ?? []).map((item) => [
            item.id,
            item,
          ]),
        );
        return produce(resources, (draft) => {
          draft.route = resources?.[ADCSDK.ResourceType.ROUTE]?.map((route) =>
            produce(route, (routeDraft) => {
              {
                if (route.plugin_config_id)
                  routeDraft.plugins =
                    pluginConfigIdMap[route.plugin_config_id].plugins;
              }
            }),
          );
        });
      }),
      // Move upstreams to service or route
      map((resources) => {
        const upstreamIdMap = Object.fromEntries(
          (resources?.upstream ?? []).map((item) => [
            item.id,
            this.toADC.transformUpstream(item),
          ]),
        );

        // If upstreams are explicitly specified with associated service, index them separately
        const upstreamServiceIdMap = resources?.upstream?.reduce<
          Record<string, ADCSDK.Upstream[]>
        >((pv, cv) => {
          const serviceId = cv.labels?.[
            typing.ADC_UPSTREAM_SERVICE_ID_LABEL
          ] as string;
          if (serviceId) {
            if (!pv[serviceId]) pv[serviceId] = [];
            pv[serviceId].push(upstreamIdMap[cv.id]);
          }
          return pv;
        }, {});
        return produce(resources, (draft) => {
          draft.route = resources?.[ADCSDK.ResourceType.ROUTE]?.map((route) =>
            produce(route, (routeDraft) => {
              if (route.upstream_id)
                routeDraft.upstream = upstreamIdMap[route.upstream_id];
            }),
          );
          draft.service = resources?.[ADCSDK.ResourceType.SERVICE]?.map(
            (service) =>
              produce(service, (serviceDraft) => {
                if (service.upstream_id)
                  serviceDraft.upstream = upstreamIdMap[service.upstream_id];
                unset(serviceDraft, 'upstream.id');
                unset(serviceDraft, 'upstream.name');
                if (upstreamServiceIdMap?.[service.id])
                  serviceDraft.upstreams = upstreamServiceIdMap[service.id];
              }),
          );
        });
      }),
      switchMap((resources) => {
        return of({
          ssls: resources?.[ADCSDK.ResourceType.SSL]?.map((ssl) =>
            this.toADC.transformSSL(ssl),
          ),
          consumers: resources?.[ADCSDK.ResourceType.CONSUMER]?.map(
            (consumer) => this.toADC.transformConsumer(consumer, true),
          ),
          global_rules: resources[ADCSDK.ResourceType.GLOBAL_RULE],
          plugin_metadata: resources[ADCSDK.ResourceType.PLUGIN_METADATA],
        } as ADCSDK.Configuration).pipe(
          // Move routes and stream_routes to service
          map((config) => {
            const serviceIdMap = Object.fromEntries(
              (resources?.service ?? []).map((item) => [
                item.id,
                this.toADC.transformService(item),
              ]),
            );
            resources?.route?.forEach((item) => {
              const route = this.toADC.transformRoute(item);
              if (item.service_id) {
                if (!serviceIdMap[item.service_id]) return; //TODO error report
                if (!serviceIdMap[item.service_id].routes)
                  serviceIdMap[item.service_id].routes = [];
                serviceIdMap[item.service_id].routes?.push(route);
              }
            });
            resources?.stream_route?.forEach((item) => {
              const route = this.toADC.transformStreamRoute(item);
              if (item.service_id) {
                if (!serviceIdMap[item.service_id]) return; //TODO error report
                if (!serviceIdMap[item.service_id].stream_routes)
                  serviceIdMap[item.service_id].stream_routes = [];
                serviceIdMap[item.service_id].stream_routes?.push(route);
              }
            });
            return produce(config, (draft) => {
              draft.services = Object.values(serviceIdMap).map((item) =>
                ADCSDK.utils.recursiveOmitUndefined({
                  ...item,
                  routes:
                    item?.routes?.length || 0 > 0 ? item?.routes : undefined,
                  stream_routes:
                    item?.stream_routes?.length || 0 > 0
                      ? item.stream_routes
                      : undefined,
                }),
              );
            });
          }),
        );
      }),
    );
  }
}
