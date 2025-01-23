import { Command } from 'commander';
import dotenv from 'dotenv';

import { ConvertCommand } from './convert.command';
import { DevCommand } from './dev.command';
import { DiffCommand } from './diff.command';
import { DumpCommand } from './dump.command';
import { LintCommand } from './lint.command';
import { PingCommand } from './ping.command';
import { SyncCommand } from './sync.command';
import { configurePluralize } from './utils';

export const setupCommands = (): Command => {
  const program = new Command('adc');

  program
    .description(
      'API Declarative CLI (ADC) is a utility to manage API7 Enterprise and Apache APISIX declaratively.\n\nLearn more at: https://docs.api7.ai/enterprise/reference/adc',
    )
    .configureHelp({ showGlobalOptions: true })
    .passThroughOptions()
    .version('0.17.0', '-v, --version', 'display ADC version');

  if (
    process.env.ADC_EXPERIMENTAL_FEATURE_FLAGS &&
    process.env.ADC_EXPERIMENTAL_FEATURE_FLAGS.includes('remote-state-cache')
  ) {
    const desc =
      'path of the remote state file, which will allow the ADC to skip the initial dump process and use the ADC configuration contained in the remote state file directly';
    DiffCommand.option('--remote-state-file <file-path>', desc);
    SyncCommand.option('--remote-state-file <file-path>', desc);
  }

  program
    .addCommand(PingCommand)
    .addCommand(DumpCommand)
    .addCommand(DiffCommand)
    .addCommand(SyncCommand)
    .addCommand(ConvertCommand)
    .addCommand(LintCommand);
  //.addCommand(ValidateCommand)

  if (process.env.NODE_ENV === 'development') program.addCommand(DevCommand);

  // initialize dotenv
  dotenv.config();

  // initialize pluralize
  configurePluralize();

  return program;
};
