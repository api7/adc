import * as ADCSDK from '@api7/adc-sdk';
import chalk from 'chalk';
import { Command, InvalidArgumentError, Option } from 'commander';
import { has } from 'lodash';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import parseDuration from 'parse-duration';
import qs from 'qs';

export interface BaseOptions {
  verbose: number;
}
export class BaseCommand<
  OPTS extends BaseOptions = BaseOptions,
> extends Command {
  private examples: Array<{ title: string; command: string }> = [];

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

  // Appends the provided examples to description
  public addExamples(examples: Array<{ title: string; command: string }>) {
    this.examples.push(...examples);

    // Title of each example is a comment which describes the actual command
    const exampleText = this.examples
      .map((example) => `  # ${example.title}\n  ${example.command}`)
      .join('\n\n');

    const exampleHeader = this.examples.length === 1 ? 'Example:' : 'Examples:';

    const currDescription = this.description() || '';

    // Append the examples to the description
    this.description(`${currDescription}\n\n${exampleHeader}\n${exampleText}`);
    return this;
  }

  public handle(cb: (opts: OPTS, command: Command) => void | Promise<void>) {
    this.action((_, command: Command) => cb(command.opts<OPTS>(), command));
    return this;
  }
}

export class BackendCommand<
  OPTS extends BaseOptions = BaseOptions,
> extends BaseCommand<OPTS> {
  constructor(name: string, summary?: string, description?: string) {
    super(name, summary, description);

    this.addBackendOptions();
  }

  public handle(func: (opts: OPTS, command: Command) => void | Promise<void>) {
    return super.handle(async (opts, command) => {
      if (
        (has(opts, 'tlsClientCertFile') && !has(opts, 'tlsClientKeyFile')) ||
        (!has(opts, 'tlsClientCertFile') && has(opts, 'tlsClientKeyFile'))
      ) {
        console.log(
          chalk.red(
            'TLS client certificate and key must be provided at the same time',
          ),
        );
        return;
      }
      await func(opts, command);
    });
  }

  private addBackendOptions() {
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
        .choices(['apisix', 'api7ee', 'apisix-standalone'])
        .default('apisix'),
    )
      .addOption(
        new Option('--server <string>', 'HTTP address of the backend')
          .env('ADC_SERVER')
          .default('http://localhost:9180'),
      )
      .addOption(
        new Option(
          '--token <string>',
          'token for ADC to connect to the backend',
        ).env('ADC_TOKEN'),
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

export const processCertificateFile = (value: string, err: string) => {
  const path = resolve(value);
  if (!existsSync(path)) throw new InvalidArgumentError(err);
  return path;
};
export const NoLintOption = new Option('--no-lint', 'disable lint check');
export const RequestConcurrentOption = new Option(
  '--request-concurrent <integer>',
  'number of concurrent requests to the backend',
)
  .default(10, '10')
  .argParser((val) => {
    const int = parseInt(val);
    if (!Number.isInteger(int))
      throw new InvalidArgumentError('Not an integer');
    return int;
  });
