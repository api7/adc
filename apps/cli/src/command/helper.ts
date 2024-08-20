import * as ADCSDK from '@api7/adc-sdk';
import chalk from 'chalk';
import { Command, InvalidArgumentError, Option } from 'commander';
import { has } from 'lodash';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import parseDuration from 'parse-duration';
import qs from 'qs';

export class BaseCommand extends Command {
  private exmaples: Array<string> = [];

  constructor(name: string, summary?: string, description?: string) {
    super(name);

    if (summary) this.summary(summary);
    if (description) this.description(description);

    // Add global flag - verbose
    this.addOption(
      new Option(
        '--verbose <integer>',
        'set the verbosity level for logs (0: no logs, 1: basic logs, 2: debug logs)',
      )
        .argParser((val) => {
          const int = parseInt(val);
          if (!Number.isInteger(int)) return 1;
          if (int >= 0 && int <= 2) return int;
          return int < 0 ? 0 : 2;
        })
        .default(1),
    );
  }

  public addExample(exmaple: string) {
    if (this.exmaples.length === 0)
      this.addHelpText('after', () => {
        return `\nExamples:\n\n${this.exmaples
          .map((example) => `  $ ${example}`)
          .join('\n')}\n`;
      });

    this.exmaples.push(exmaple);
    return this;
  }
}

export class BackendCommand<OPTS extends object = object> extends BaseCommand {
  constructor(name: string, summary?: string, description?: string) {
    super(name, summary, description);

    this.addBackendOptions();
  }

  public handle(cb: (opts: OPTS, command: Command) => void | Promise<void>) {
    this.action(async (_, command: Command) => {
      const opts = command.opts<OPTS>();

      if (
        (has(opts, 'tlsClientCertFile') && !has(opts, 'tlsClientKeyFile')) ||
        (has(opts, 'tlsClientKeyFile') && !has(opts, 'tlsClientCertFile'))
      ) {
        console.log(
          chalk.red(
            'TLS client certificate and key must be provided at the same time',
          ),
        );
        return;
      }

      await cb(opts, command);
    });
    return this;
  }

  private addBackendOptions() {
    const processCertificateFile = (value: string, err: string) => {
      const path = resolve(value);
      if (!existsSync(path)) throw new Error(err);
      return path;
    };

    const parseResourceTypeFilter = (
      cv: ADCSDK.ResourceType,
      pv: Array<string> = [],
    ) => {
      const resourceTypes = Object.values(ADCSDK.ResourceType);
      if (!resourceTypes.includes(cv))
        throw new InvalidArgumentError(
          `Allowed choices are ${resourceTypes.join(', ')}.`,
        );
      return pv.concat(cv);
    };

    this.addOption(
      new Option('--backend <backend>', 'type of backend to connect to')
        .env('ADC_BACKEND')
        .choices(['apisix', 'api7ee'])
        .default('apisix'),
    )
      .addOption(
        new Option(
          '--server <string>',
          'HTTP address of the backend',
        )
          .env('ADC_SERVER')
          .default('http://localhost:9180'),
      )
      .addOption(
        new Option('--token <string>', 'token for ADC to connect to the backend').env(
          'ADC_TOKEN',
        ),
      )
      .addOption(
        new Option(
          '--gateway-group <string>',
          'gateway group to operate on (only supported for "api7ee" backend)',
        )
          .env('ADC_GATEWAY_GROUP')
          .default('default'),
      )
      .addOption(
        new Option(
          '--label-selector <labelKey=labelValue>',
          'filter resources by labels',
        ).argParser((val, previous: Record<string, string> = {}) =>
          Object.assign(previous, qs.parse(val, { delimiter: ',' })),
        ),
      )
      .addOption(
        new Option(
          '--include-resource-type <string>',
          'filter resources that only contains the specified type',
        )
          .conflicts('excludeResourceType')
          .choices(Object.values(ADCSDK.ResourceType))
          .argParser(parseResourceTypeFilter),
      )
      .addOption(
        new Option(
          '--exclude-resource-type <string>',
          'filter resources that does not contain the specified type',
        )
          .conflicts('includeResourceType')
          .choices(Object.values(ADCSDK.ResourceType))
          .argParser(parseResourceTypeFilter),
      )
      .addOption(
        new Option(
          '--timeout <duration>',
          'timeout for adc to connect with the backend (examples: 10s, 1h10m)',
        )
          .default(10000, '10s')
          .argParser((value) => {
            return parseDuration(value) ?? 10000;
          }),
      )
      .addOption(
        new Option(
          '--ca-cert-file <string>',
          'path to the CA certificate to verify the backend',
        )
          .env('ADC_CA_CERT_FILE')
          .argParser((value) =>
            processCertificateFile(
              value,
              'The specified CA certificate file does not exist',
            ),
          ),
      )
      .addOption(
        new Option(
          '--tls-client-cert-file <string>',
          'path to the mutual TLS client certificate to verify the backend',
        )
          .env('ADC_TLS_CLIENT_CERT_FILE')
          .argParser((value) =>
            processCertificateFile(
              value,
              'The specified mutual TLS client certificate file does not exist',
            ),
          ),
      )
      .addOption(
        new Option(
          '--tls-client-key-file <string>',
          'path to the mutual TLS client key to verify the backend',
        )
          .env('ADC_TLS_CLIENT_KEY_FILE')
          .argParser((value) =>
            processCertificateFile(
              value,
              'The specified mutual TLS client key file does not exist',
            ),
          ),
      )
      .addOption(
        new Option(
          '--tls-skip-verify',
          `disable the verification of the backend TLS certificate`,
        )
          .env('ADC_TLS_SKIP_VERIFY')
          .default(false),
      );
  }
}

export const NoLintOption = new Option('--no-lint', 'disable lint check');
