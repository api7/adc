import * as ADCSDK from '@api7/adc-sdk';
import axios, { type AxiosInstance } from 'axios';
import type { JSONSchema4 } from 'json-schema';
import { Observable, Subject, forkJoin, from, switchMap } from 'rxjs';
import semver, { SemVer } from 'semver';

import { Fetcher } from './fetcher';
import { Operator } from './operator';

export class BackendAPISIX implements ADCSDK.Backend {
  private static logScope = ['APISIX'];
  private readonly client: AxiosInstance;
  private readonly subject = new Subject<ADCSDK.BackendEvent>();

  private _version?: SemVer;
  private _pluginSchemas?: ADCSDK.PluginSchemaMap;

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    this.client = axios.create({
      baseURL: `${opts.server}`,
      headers: {
        'Content-Type': 'application/json',
        'X-API-KEY': opts.token,
      },
      httpAgent: opts.httpAgent,
      httpsAgent: opts.httpsAgent,
      ...(opts.timeout ? { timeout: opts.timeout } : {}),
    });
  }

  public metadata() {
    return {
      logScope: BackendAPISIX.logScope,
    };
  }

  public async defaultValue() {
    return {};
  }

  public async ping(): Promise<void> {
    await this.client.get(`/apisix/admin/routes`);
  }

  public async version(): Promise<SemVer> {
    if (this._version) return this._version;

    const resp = await this.client.get<{ value: string }>(
      '/apisix/admin/routes',
    );
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: resp, description: `Get APISIX version` },
    });

    const fallback = new semver.SemVer('999.999.999');
    this._version = fallback;
    if (resp.headers.server) {
      const version = (resp.headers.server as string).match(/APISIX\/(.*)/);
      if (version) this._version = semver.coerce(version[1]) ?? fallback;
    }

    return this._version;
  }

  public dump(): Observable<ADCSDK.Configuration> {
    return forkJoin([
      from(this.version()),
      from(this.defaultValue()),
    ]).pipe<ADCSDK.Configuration>(
      switchMap(([version]) => {
        const fetcher = new Fetcher({
          client: this.client,
          version: version,
          eventSubject: this.subject,
          backendOpts: this.opts,
        });
        return fetcher.dump();
      }),
    );
  }

  public sync(
    events: Array<ADCSDK.Event>,
    opts: ADCSDK.BackendSyncOptions = { exitOnFailure: true },
  ): Observable<ADCSDK.BackendSyncResult> {
    return forkJoin([from(this.version()), from(this.defaultValue())]).pipe(
      switchMap(([version]) => {
        return new Operator({
          client: this.client,
          version,
          eventSubject: this.subject,
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

  supportValidate?: () => Promise<boolean>;
  supportStreamRoute?: () => Promise<boolean>;

  public async fetchPluginSchemas(): Promise<ADCSDK.PluginSchemaMap> {
    if (this._pluginSchemas) return this._pluginSchemas;

    // Fetch plugin list
    const listResp = await this.client.get<string[]>(
      '/apisix/admin/plugins/list',
    );
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: listResp, description: 'Get plugins list' },
    });
    const pluginNames = listResp.data ?? [];

    // Fetch schema for each plugin
    const schemas: ADCSDK.PluginSchemaMap = {};
    await Promise.all(
      pluginNames.map(async (name) => {
        try {
          const resp = await this.client.get<JSONSchema4>(
            `/apisix/admin/schema/plugins/${name}`,
          );
          this.subject.next({
            type: ADCSDK.BackendEventType.AXIOS_DEBUG,
            event: {
              response: resp,
              description: `Get plugin schema: ${name}`,
            },
          });
          const entry: ADCSDK.PluginSchemaEntry = {
            configSchema: resp.data,
          };

          // Fetch consumer schema if applicable
          try {
            const consumerResp = await this.client.get<JSONSchema4>(
              `/apisix/admin/schema/plugins/${name}`,
              { params: { schema_type: 'consumer' } },
            );
            if (consumerResp.data && Object.keys(consumerResp.data).length > 0)
              entry.consumerSchema = consumerResp.data;
          } catch {
            // Plugin doesn't have a consumer schema — skip
          }

          schemas[name] = entry;
        } catch {
          // Skip plugins whose schema cannot be fetched
        }
      }),
    );

    this._pluginSchemas = schemas;
    return schemas;
  }
}
