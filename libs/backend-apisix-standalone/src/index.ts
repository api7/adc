import * as ADCSDK from '@api7/adc-sdk';
import axios, { type AxiosInstance } from 'axios';
import { type Observable, Subject, from, map, of, switchMap } from 'rxjs';
import semver, { SemVer, eq as semverEQ } from 'semver';

import {
  config as configCache,
  rawConfig as rawConfigCache,
  version as versionCache,
} from './cache';
import { Fetcher } from './fetcher';
import { Operator } from './operator';
import * as typing from './typing';

export class BackendAPISIXStandalone implements ADCSDK.Backend {
  private static logScope = ['APISIX'];
  private readonly client: AxiosInstance;
  private readonly subject = new Subject<ADCSDK.BackendEvent>();

  private readonly serverTokenMap: typing.ServerTokenMap = new Map<
    string,
    string
  >();

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    const servers = opts.server.split(',');
    const tokens = opts.token.split(',');
    servers.forEach((server, idx) => {
      this.serverTokenMap.set(
        server,
        tokens[tokens.length === servers.length ? idx : 0],
      );
    });

    this.client = axios.create({
      headers: { 'Content-Type': 'application/json' },
      httpAgent: opts.httpAgent,
      httpsAgent: opts.httpsAgent,
      ...(opts.timeout ? { timeout: opts.timeout } : {}),
    });
  }

  public metadata() {
    return {
      logScope: BackendAPISIXStandalone.logScope,
    };
  }

  public async defaultValue() {
    return {};
  }

  public async ping(): Promise<void> {
    await this.client.head(`/apisix/admin`);
  }

  public async version() {
    const cachedVersion = versionCache.get(this.opts.cacheKey);
    if (cachedVersion) return cachedVersion;

    const resp = await this.client.head<{ value: string }>(
      // Always get the version from the first server
      `${this.serverTokenMap.keys().next().value}/apisix/admin`,
    );
    this.subject.next({
      type: ADCSDK.BackendEventType.AXIOS_DEBUG,
      event: { response: resp, description: `Get APISIX version` },
    });

    const mockVersion = '999.999.999';
    let version = new SemVer(mockVersion);
    if (resp.headers.server) {
      const parsedVersion = (resp.headers.server as string).match(
        /APISIX\/(.*)/,
      );
      if (parsedVersion) version = semver.coerce(parsedVersion[1]) ?? version;
    }

    // Only cache it when the actual value is obtained
    if (!semverEQ(version, mockVersion))
      versionCache.set(this.opts.cacheKey, version);

    return version;
  }

  // Get a snapshot of the remote configuration that should be cached
  // 1. If there is a cached configuration for a particular cacheKey, return it directly
  // 2. Otherwise initialize it, following the process:
  // 2.1. Requests each of servers and gets the timestamp they were last updated from their response header
  // 2.2. Find the latest updated server among them and it will be used as the initial cache
  // 3. Transform and return that configuration
  public dump(): Observable<ADCSDK.Configuration> {
    return from(this.version()).pipe<ADCSDK.Configuration>(
      switchMap((version) => {
        const cachedConfig = configCache.get(this.opts.cacheKey);
        if (cachedConfig) return of(cachedConfig);

        // Initialize config cache
        // The following logic is designed to run only for the initial dump of each cacheKey.
        // Other than that, dump always uses the cache, and its value is updated at each sync.
        const fetcher = new Fetcher({
          client: this.client,
          serverTokenMap: this.serverTokenMap,
          version: version,
          eventSubject: this.subject,
          backendOpts: this.opts,
        });
        return from(fetcher.dump()).pipe(
          map(
            ([config, rawConfig]) => (
              configCache.set(this.opts.cacheKey, config),
              rawConfigCache.set(this.opts.cacheKey, rawConfig),
              config
            ),
          ),
        );
      }),
    );
  }

  // Derive the latest configuration from the cached raw config via a change event,
  // sending the new configuration to each server
  // 1. Modify the raw config based on events to accomplish resource creation, update and deletion
  // 2. Generate a digest string for the new configuration
  // 3. Send a PUT request containing the new configuration to the API of each server
  // 4. Once again, the raw config just sent is parsed into an ADC configuration and these two latest configurations are cached
  // 5. Report sync results to the ADC
  public sync(
    events: Array<ADCSDK.Event>,
    opts: ADCSDK.BackendSyncOptions = { exitOnFailure: true },
  ): Observable<ADCSDK.BackendSyncResult> {
    return from(this.version()).pipe(
      switchMap((version) => {
        return new Operator({
          cacheKey: this.opts.cacheKey,
          client: this.client,
          serverTokenMap: this.serverTokenMap,
          version,
          eventSubject: this.subject,
          oldRawConfiguration: rawConfigCache.get(this.opts.cacheKey) ?? {},
        }).sync(events, opts);
      }),
    );
  }

  public on(
    eventType: keyof typeof ADCSDK.BackendEventType,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    cb: (...args: any[]) => void,
  ) {
    return this.subject.subscribe(({ type, event }) => {
      if (eventType === type) cb(event);
    });
  }

  supportValidate?: () => Promise<boolean>;
  supportStreamRoute?: () => Promise<boolean>;

  public __TEST_ONLY = {
    GET_CLIENT: () => this.client,
  };
}
