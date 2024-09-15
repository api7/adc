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
    .description('API Declarative CLI (ADC) is a utility to manage API7 Enterprise and Apache APISIX declaratively.\n\nLearn more at: https://docs.api7.ai/enterprise/reference/adc')
    .configureHelp({ showGlobalOptions: true })
    .passThroughOptions()
    .version('0.14.0', '-v, --version', 'display ADC version');

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
