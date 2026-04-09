import * as ADCSDK from '@api7/adc-sdk';
import { Option } from 'commander';
import { Listr } from 'listr2';

import {
  FetchPluginSchemasTask,
  LintTask,
  LoadLocalConfigurationTask,
} from '../tasks';
import { InitializeBackendTask } from '../tasks/init_backend';
import { SignaleRenderer } from '../utils/listr';
import { BaseCommand } from './helper';

export const LintCommand = new BaseCommand('lint')
  .description(
    'Lint the local configuration file(s) to ensure it meets ADC requirements.\n\nOptionally, provide backend connection parameters to also validate plugin configurations against the backend\'s plugin schemas.',
  )
  .summary('lint the local configuration')
  .option(
    '-f, --file <file-path>',
    'file to lint',
    (filePath, files: Array<string> = []) => files.concat(filePath),
  )
  .addOption(
    new Option('--backend <backend>', 'type of backend to validate plugins against')
      .choices(['apisix', 'api7ee']),
  )
  .addOption(
    new Option('--server <string>', 'HTTP address of the backend'),
  )
  .addOption(
    new Option(
      '--token <string>',
      'token for ADC to connect to the backend',
    ),
  )
  .addOption(
    new Option(
      '--gateway-group <string>',
      'gateway group to operate on (only for "api7ee" backend)',
    )
      .default('default'),
  )
  .addExamples([
    {
      title: 'Lint the specified configuration file',
      command: 'adc lint -f adc.yaml',
    },
    {
      title: 'Lint multiple configuration files',
      command: 'adc lint -f service-a.yaml -f service-b.yaml',
    },
    {
      title: 'Lint with plugin validation against an APISIX backend',
      command: 'adc lint -f adc.yaml --backend apisix --server http://localhost:9180 --token edd1c9f034335f136f87ad84b625c8f1',
    },
    {
      title: 'Lint with plugin validation against an API7 EE backend',
      command: 'adc lint -f adc.yaml --backend api7ee --server https://dashboard.example.com --token <token>',
    },
  ])
  .action(async () => {
    const opts = LintCommand.optsWithGlobals();
    const useBackend = !!opts.backend;

    const tasks = new Listr(
      [
        ...(useBackend
          ? [
              InitializeBackendTask(opts.backend, {
                ...opts,
                cacheKey: 'lint',
              } as ADCSDK.BackendOptions),
              FetchPluginSchemasTask(),
            ]
          : []),
        LoadLocalConfigurationTask(opts.file, {}),
        LintTask(),
      ],
      {
        renderer: SignaleRenderer,
        rendererOptions: { verbose: opts.verbose },
      },
    );

    try {
      await tasks.run();
    } catch {
      process.exit(1);
    }
  });
