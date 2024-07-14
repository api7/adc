import * as ADCSDK from '@api7/adc-sdk';
import axios, { Axios, CreateAxiosDefaults } from 'axios';
import { Listr, ListrTask } from 'listr2';
import { readFileSync } from 'node:fs';
import { AgentOptions, Agent as httpsAgent } from 'node:https';

import { Fetcher } from './fetcher';
import { Operator } from './operator';

export class BackendAPISIX implements ADCSDK.Backend {
  private readonly client: Axios;
  private static logScope = ['APISIX'];

  constructor(private readonly opts: ADCSDK.BackendOptions) {
    const config: CreateAxiosDefaults = {
      baseURL: `${opts.server}`,
      headers: {
        'Content-Type': 'application/json',
        'X-API-KEY': opts.token,
      },
    };

    if (opts.server.startsWith('https')) {
      const agentConfig: AgentOptions = {
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

  public async ping(): Promise<void> {
    await this.client.get(`/apisix/admin/routes`);
  }

  public getResourceDefaultValueTask(): Array<ListrTask> {
    return [];
  }

  public async dump(): Promise<Listr<{ remote: ADCSDK.Configuration }>> {
    const fetcher = new Fetcher(this.client);
    return new Listr(
      [...this.getResourceDefaultValueTask(), ...fetcher.fetch()],
      {
        rendererOptions: { scope: BackendAPISIX.logScope },
      },
    );
  }

  public async sync(): Promise<Listr> {
    const operator = new Operator(this.client);
    return new Listr(
      [
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
