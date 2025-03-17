import * as ADCSDK from '@api7/adc-sdk';
import axios, { Axios, CreateAxiosDefaults } from 'axios';
import { Listr, ListrTask } from 'listr2';
import { readFileSync } from 'node:fs';
import {
  Agent as httpAgent,
  AgentOptions as httpAgentOptions,
} from 'node:http';
import {
  Agent as httpsAgent,
  AgentOptions as httpsAgentOptions,
} from 'node:https';
import { Observable, Subscription } from 'rxjs';
import semver from 'semver';

import { Fetcher } from './fetcher';
import { Operator } from './operator';
import { buildReqAndRespDebugOutput } from './utils';

export class BackendAPISIX implements ADCSDK.Backend {
  private readonly client: Axios;
  private static logScope = ['APISIX'];

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    const keepAlive: httpAgentOptions = {
      keepAlive: true,
      keepAliveMsecs: 60000,
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

  on(eventType: unknown, cb: unknown): Subscription {
    //@ts-expect-errordddd
    return {};
  }
  defaultValue: () => Promise<ADCSDK.DefaultValue>;
  dump: () => Observable<ADCSDK.Configuration>;
  sync: (events: Array<ADCSDK.Event>) => Observable<ADCSDK.BackendSyncResult>;

  public async ping(): Promise<void> {
    await this.client.get(`/apisix/admin/routes`);
  }

  private getAPISIXVersionTask(): ListrTask {
    return {
      enabled: (ctx) => !ctx.apisixVersion,
      task: async (ctx, task) => {
        const resp = await this.client.get<{ value: string }>(
          '/apisix/admin/routes',
        );
        task.output = buildReqAndRespDebugOutput(resp, `Get APISIX version`);

        ctx.apisixVersion = semver.coerce('0.0.0');
        if (resp.headers.server) {
          const version = (resp.headers.server as string).match(/APISIX\/(.*)/);
          if (version) ctx.apisixVersion = semver.coerce(version[1]);
        }
      },
    };
  }

  public getResourceDefaultValueTask(): Array<ListrTask> {
    return [];
  }

  public async dump0(): Promise<Listr<{ remote: ADCSDK.Configuration }>> {
    const fetcher = new Fetcher(this.client);
    return new Listr(
      [
        this.getAPISIXVersionTask(),
        ...this.getResourceDefaultValueTask(),
        ...fetcher.fetch(),
      ],
      {
        rendererOptions: { scope: BackendAPISIX.logScope },
      },
    );
  }

  public async sync0(): Promise<Listr> {
    const operator = new Operator(this.client);
    return new Listr(
      [
        this.getAPISIXVersionTask(),
        ...this.getResourceDefaultValueTask(),
        {
          task: (ctx, task) =>
            task.newListr(
              ctx.diff.map((event: ADCSDK.Event) =>
                event.type === ADCSDK.EventType.DELETE
                  ? operator.deleteResource(event)
                  : operator.updateResource(event),
              ),
            ),
        },
      ],
      {
        rendererOptions: { scope: BackendAPISIX.logScope },
      },
    );
  }

  supportValidate?: () => Promise<boolean>;
  supportStreamRoute?: () => Promise<boolean>;
}
