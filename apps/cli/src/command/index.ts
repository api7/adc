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
    .configureHelp({ showGlobalOptions: true })
    .passThroughOptions()
    .version('0.11.1');

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
