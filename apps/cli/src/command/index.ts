import * as ADCSDK from '@api7/adc-sdk';
import { Command } from 'commander';
import dotenv from 'dotenv';

import { ConvertCommand } from './convert.command';
import { DevCommand } from './dev.command';
import { DiffCommand } from './diff.command';
import { DumpCommand } from './dump.command';
import { IngressSyncCommand } from './ingress-sync.command';
import { LintCommand } from './lint.command';
import { PingCommand } from './ping.command';
import { SyncCommand } from './sync.command';
import { configurePluralize } from './utils';

const versionCode = '0.19.1';

// initialize dotenv
dotenv.config();

// initialize pluralize
configurePluralize();

export const setupCommands = (): Command => {
  const program = new Command('adc');

  program
    .description(
      'API Declarative CLI (ADC) is a utility to manage API7 Enterprise and Apache APISIX declaratively.\n\nLearn more at: https://docs.api7.ai/enterprise/reference/adc',
    )
    .configureHelp({ showGlobalOptions: true })
    .passThroughOptions()
    .version(versionCode, '-v, --version', 'display ADC version');

  if (
    ADCSDK.utils.featureGateEnabled(ADCSDK.utils.featureGate.REMOTE_STATE_FILE)
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

  return program;
};

export const setupIngressCommands = (): Command => {
  const program = new Command('adc-ingress');

  program
    .description('API Declarative CLI (ADC) for Ingress Controller')
    .configureHelp({ showGlobalOptions: true })
    .passThroughOptions()
    .version(versionCode, '-v, --version', 'display ADC version');

  if (
    ADCSDK.utils.featureGateEnabled(ADCSDK.utils.featureGate.REMOTE_STATE_FILE)
  ) {
    const desc =
      'path of the remote state file, which will allow the ADC to skip the initial dump process and use the ADC configuration contained in the remote state file directly';
    DiffCommand.option('--remote-state-file <file-path>', desc);
    SyncCommand.option('--remote-state-file <file-path>', desc);
  }

  program.addCommand(IngressSyncCommand);

  if (process.env.NODE_ENV === 'development') program.addCommand(DevCommand);

  return program;
};
