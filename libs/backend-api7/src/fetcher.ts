import * as ADCSDK from '@api7/adc-sdk';
import { Axios } from 'axios';
import { produce } from 'immer';
import { curry, isEmpty } from 'lodash';
import EventEmitter from 'node:events';
import { Subject, combineLatest, from, map, mergeMap, of, takeLast, tap, toArray } from 'rxjs';
import { SemVer } from 'semver';



import { ToADC } from './transformer';
import * as typing from './typing';


type TaskSubjectInfo = {
  d: object;
  desc?: string;
};
type TaskSubject = {
  name: string;
  event: keyof typeof ADCSDK.BackendEventType;
  info: TaskSubjectInfo;
};
const taskMap = new Map<TaskSubject['name'], TaskSubject[]>();
const taskSubject = new Subject<TaskSubject>();
const _genTaskLog = (
  name: string,
  event: keyof typeof ADCSDK.BackendEventType,
  info: TaskSubjectInfo,
) => {
  taskSubject.next({ name, event, info });
};
const genTaskLog = curry(_genTaskLog);
export interface FetcherOptions {
  client: Axios;
  version: SemVer;
  eventEmitter: EventEmitter;
  backendOpts: ADCSDK.BackendOptions;
  gatewayGroupName?: string;
  gatewayGroupId?: string;
}
export class Fetcher {
  private readonly toADC = new ToADC();
  private readonly client: Axios;
  private readonly eventEmitter: EventEmitter;

  constructor(private readonly opts: FetcherOptions) {
    this.client = opts.client;
    this.eventEmitter = opts.eventEmitter;
  }

  public listServices() {
    if (this.isSkip(ADCSDK.ResourceType.SERVICE))
      return of<Array<ADCSDK.Service>>([]);

    const taskLog = genTaskLog('Fetch services');
    taskLog('TASK_START', { d: {} });
    return from(
      this.client.get<typing.ListResponse<typing.Service>>(
        `/apisix/admin/services`,
        {
          params: this.attachLabelSelector({
            gateway_group_id: this.opts.gatewayGroupId,
          }),
        },
      ),
    ).pipe(
      tap((resp) => {
        taskLog('AXIOS_DEBUG', { d: resp });
      }),
      mergeMap((resp) =>
        from(resp.data.list).pipe(
          mergeMap((service) => {
            const params = {
              gateway_group_id: this.opts.gatewayGroupId,
              service_id: service.id,
            };
            return from(
              service.type === 'stream'
                ? this.client.get<typing.ListResponse<typing.StreamRoute>>(
                    `/apisix/admin/stream_routes`,
                    { params },
                  )
                : this.client.get<typing.ListResponse<typing.Route>>(
                    `/apisix/admin/routes`,
                    { params },
                  ),
            ).pipe(
              tap((resp) => {
                taskLog('AXIOS_DEBUG', {
                  d: resp,
                  desc: `Get ${service.type === 'stream' ? 'stream routes' : 'routes'} in service "${service.name}"`,
                });
              }),
              map((resp) => {
                const routes = resp.data.list;
                return produce(service, (draft) => {
                  if (service.type === 'stream')
                    draft.stream_routes = routes as typing.StreamRoute[];
                  if (service.type === 'http')
                    service.routes = routes as typing.Route[];
                });
              }),
            );
          }),
          map((service) => this.toADC.transformService(service)),
        ),
      ),
      toArray(),
      tap(() => {
        taskLog('TASK_DONE', { d: {} });
      }),
    );
  }

  public listConsumers() {
    if (this.isSkip(ADCSDK.ResourceType.CONSUMER))
      return of<Array<ADCSDK.Consumer>>([]);
    const taskLog = genTaskLog('Fetch consumers');
    taskLog('TASK_START', { d: {} });
    return from(
      this.client.get<{ list: Array<typing.Consumer> }>(
        '/apisix/admin/consumers',
        {
          params: this.attachLabelSelector({
            gateway_group_id: this.opts.gatewayGroupId,
          }),
        },
      ),
    ).pipe(
      tap((resp) => {
        taskLog('AXIOS_DEBUG', { d: resp });
      }),
      mergeMap((resp) =>
        from(resp.data.list).pipe(
          mergeMap((consumer) =>
            from(
              this.client.get<{
                list: Array<typing.ConsumerCredential>;
              }>(`/apisix/admin/consumers/${consumer.username}/credentials`, {
                // In the current design, the consumer's credentials are not filtered
                // using labels because nested labels filters can be misleading. Even
                // if labels set for the consumer, the labels filter is not attached.
                params: { gateway_group_id: this.opts.gatewayGroupId },
              }),
            ).pipe(
              tap((resp) => {
                taskLog('AXIOS_DEBUG', {
                  d: resp,
                  desc: `Get credentials of consumer "${consumer.username}"`,
                });
              }),
              map((resp) =>
                produce(consumer, (draft) => {
                  draft.credentials = resp?.data?.list;
                }),
              ),
            ),
          ),
        ),
      ),
      map((consumer) => this.toADC.transformConsumer(consumer)),
      toArray(),
      tap(() => {
        taskLog('TASK_DONE', { d: {} });
      }),
    );
  }

  public listSSLs() {
    if (this.isSkip(ADCSDK.ResourceType.SSL)) return of<Array<ADCSDK.SSL>>([]);

    const taskLog = genTaskLog('Fetch ssls');
    taskLog('TASK_START', { d: {} });
    return from(
      this.client.get<{ list: Array<typing.SSL> }>('/apisix/admin/ssls', {
        params: this.attachLabelSelector({
          gateway_group_id: this.opts.gatewayGroupId,
        }),
      }),
    ).pipe(
      tap((resp) => {
        taskLog('AXIOS_DEBUG', {
          d: resp,
        });
      }),
      mergeMap((resp) =>
        from(resp.data.list).pipe(map((ssl) => this.toADC.transformSSL(ssl))),
      ),
      toArray(),
      tap(() => {
        taskLog('TASK_DONE', { d: {} });
      }),
    );
  }

  public listGlobalRules() {
    if (this.isSkip(ADCSDK.ResourceType.GLOBAL_RULE))
      return of<Record<string, ADCSDK.GlobalRule>>({});

    const taskLog = genTaskLog('Fetch global rules');
    taskLog('TASK_START', { d: {} });
    return from(
      this.client.get<{ list: Array<typing.GlobalRule> }>(
        '/apisix/admin/global_rules',
        {
          params: this.attachLabelSelector({
            gateway_group_id: this.opts.gatewayGroupId,
          }),
        },
      ),
    ).pipe(
      tap((resp) => {
        taskLog('AXIOS_DEBUG', {
          d: resp,
        });
      }),
      map((resp) => this.toADC.transformGlobalRule(resp?.data?.list ?? [])),
      tap(() => {
        taskLog('TASK_DONE', { d: {} });
      }),
    );
  }

  public listMetadatas() {
    if (this.isSkip(ADCSDK.ResourceType.PLUGIN_METADATA))
      return of<Record<string, ADCSDK.PluginMetadata>>({});

    const taskLog = genTaskLog('Fetch plugin metadata');
    taskLog('TASK_START', { d: {} });
    return from(
      this.client.get<{
        value: ADCSDK.Plugins;
      }>('/apisix/admin/plugin_metadata', {
        params: this.attachLabelSelector({
          gateway_group_id: this.opts.gatewayGroupId,
        }),
      }),
    ).pipe(
      tap((resp) => {
        taskLog('AXIOS_DEBUG', {
          d: resp,
        });
      }),
      map((resp) => this.toADC.transformPluginMetadatas(resp?.data?.value)),
      tap(() => {
        taskLog('TASK_DONE', { d: {} });
      }),
    );
  }

  public allTask() {
    taskMap.clear();
    taskSubject.subscribe({
      next: (v) => {
        if (!taskMap.has(v.name)) {
          taskMap.set(v.name, [])
        }
        taskMap.get(v.name).push(v);
      }
    })
    return combineLatest([
      this.listServices(),
      this.listConsumers(),
      this.listSSLs(),
      this.listGlobalRules(),
      this.listMetadatas(),
    ]).pipe(
      takeLast(1),
      tap(() => {
        taskMap.forEach((taskSub) => {
          taskSub.forEach((v) => this.eventEmitter.emit(v.event, v.event === 'AXIOS_DEBUG'? {
            response: v.info.d,
            description: v.info.desc
          } : {
            name: v.name
          }));
        })
      }),
      map(
        ([services, consumers, ssls, global_rules, plugin_metadata]) =>
          ({
            services,
            consumers,
            ssls,
            global_rules,
            plugin_metadata,
          }) as ADCSDK.Configuration,
      ),
    );
  }

  private isSkip(type: ADCSDK.ResourceType): boolean {
    const { includeResourceType, excludeResourceType } =
      this.opts.backendOpts || {};
    if (!isEmpty(includeResourceType) && !includeResourceType.includes(type)) {
      return true;
    }
    if (!isEmpty(excludeResourceType) && excludeResourceType.includes(type)) {
      return true;
    }
    return false;
  }

  private attachLabelSelector(
    params: Record<string, string> = {},
  ): Record<string, string> {
    const { labelSelector } = this.opts.backendOpts || {};
    if (labelSelector)
      Object.entries(labelSelector).forEach(([key, value]) => {
        params[`labels[${key}]`] = value;
      });
    return params;
  }
}
