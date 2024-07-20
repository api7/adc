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

  constructor(name: string, description?: string) {
    super(name);

    if (description) this.description(description);

    // Add global flag - verbose
    this.addOption(
      new Option(
        '--verbose <integer>',
        'Override verbose logging levels, it supports 0: no logs, 1: basic logs, 2: debug logs',
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
        return `\nExample:\n\n${this.exmaples
          .map((example) => `  $ ${example}`)
          .join('\n')}\n`;
      });

    this.exmaples.push(exmaple);
    return this;
  }
}

export class BackendCommand<OPTS extends object = object> extends BaseCommand {
  constructor(name: string, description?: string) {
    super(name, description);

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
      new Option('--backend <backend>', 'Type of backend to connect')
        .env('ADC_BACKEND')
        .choices(['apisix', 'api7ee'])
        .default('apisix'),
    )
      .addOption(
        new Option(
          '--server <string>',
          'HTTP address of backend. This value can also be set using the environment variable ADC_SERVER environment variable',
        )
          .env('ADC_SERVER')
          .default('http://localhost:9180'),
      )
      .addOption(
        new Option('--token <string>', 'Token used to access the backend').env(
          'ADC_TOKEN',
        ),
      )
      .addOption(
        new Option(
          '--gateway-group <string>',
          'Gateway group used to specify the gateway group to operate [API7EE backend only]',
        )
          .env('ADC_GATEWAY_GROUP')
          .default('default'),
      )
      .addOption(
        new Option(
          '--label-selector <selectors>',
          'Filter for resource labels (e.g., labelKey=labelValue)',
        ).argParser((val, previous: Record<string, string> = {}) =>
          Object.assign(previous, qs.parse(val, { delimiter: ',' })),
        ),
      )
      .addOption(
        new Option(
          '--include-resource-type <string>',
          'Filter for resource types, contains only the specified type',
        )
          .conflicts('excludeResourceType')
          .choices(Object.values(ADCSDK.ResourceType))
          .argParser(parseResourceTypeFilter),
      )
      .addOption(
        new Option(
          '--exclude-resource-type <string>',
          'Filter for resource types, not contains only the specified type',
        )
          .conflicts('includeResourceType')
          .choices(Object.values(ADCSDK.ResourceType))
          .argParser(parseResourceTypeFilter),
      )
      .addOption(
        new Option(
          '--timeout <duration>',
          'Set a request timeout for the client to connect with Backend Admin API (in duration, e.g., 10s, 1h10m)',
        )
          .default(10000, '10s')
          .argParser((value) => {
            return parseDuration(value) ?? 10000;
          }),
      )
      .addOption(
        new Option(
          '--ca-cert-file <string>',
          'Path to CA certificate for verifying the Backend Admin API',
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
          'Path to Mutual TLS client certificate for verifying the Backend Admin API',
        )
          .env('ADC_TLS_CLIENT_CERT_FILE')
          .argParser((value) =>
            processCertificateFile(
              value,
              'The specified Mutual TLS client certificate file does not exist',
            ),
          ),
      )
      .addOption(
        new Option(
          '--tls-client-key-file <string>',
          'Path to Mutual TLS client key for verifying the Backend Admin API',
        )
          .env('ADC_TLS_CLIENT_KEY_FILE')
          .argParser((value) =>
            processCertificateFile(
              value,
              'The specified Mutual TLS client key file does not exist',
            ),
          ),
      )
      .addOption(
        new Option(
          '--tls-skip-verify',
          `Disable verification of Backend Admin API TLS certificate`,
        )
          .env('ADC_TLS_SKIP_VERIFY')
          .default(false),
      );
  }
}

export const NoLintOption = new Option('--no-lint', 'Disable lint check');
