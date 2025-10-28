import * as ADCSDK from '@api7/adc-sdk';
import axios, { type AxiosInstance, CreateAxiosDefaults } from 'axios';
import { readFileSync } from 'node:fs';
import {
  Agent as httpAgent,
  AgentOptions as httpAgentOptions,
} from 'node:http';
import {
  Agent as httpsAgent,
  AgentOptions as httpsAgentOptions,
} from 'node:https';
import { Observable, Subject, forkJoin, from, switchMap } from 'rxjs';
import semver, { SemVer } from 'semver';

import { Fetcher } from './fetcher';
import { Operator } from './operator';

export class BackendAPISIX implements ADCSDK.Backend {
  private static logScope = ['APISIX'];
  private readonly client: AxiosInstance;
  private readonly subject = new Subject<ADCSDK.BackendEvent>();

  private _version?: SemVer;

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    const keepAlive: httpAgentOptions = {
      keepAlive: true,
      keepAliveMsecs: 60000,
      maxSockets: 256, // per host
    };
    const config: CreateAxiosDefaults = {
      baseURL: `${opts.server}`,
      headers: {
        'Content-Type': 'application/json',
        'X-API-KEY': opts.token,
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

  public async version() {
    if (this._version) return this._version;

    const resp = await this.client.get<{ value: string }>(
      '/apisix/admin/routes',
    );
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: resp, description: `Get APISIX version` },
    });

    this._version = semver.coerce('999.999.999');
    if (resp.headers.server) {
      const version = (resp.headers.server as string).match(/APISIX\/(.*)/);
      if (version) this._version = semver.coerce(version[1]);
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
}
