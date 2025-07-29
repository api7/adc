import commander from 'commander';

import { ADCServer } from '../server';
import { BaseCommand, BaseOptions } from './helper';

type IngressServerOptions = {
  listen?: URL;
} & BaseOptions;

export const IngressServerCommand = new BaseCommand<IngressServerOptions>(
  'server',
)
  .option<URL>(
    '--listen <listen>',
    'listen address of ADC server, the format is scheme://host:port',
    (val) => {
      try {
        return new URL(val);
      } catch (err) {
        throw new commander.InvalidArgumentError(err);
      }
    },
    new URL('http://127.0.0.1:3000'),
  )
  .handle(async ({ listen }) => {
    await new ADCServer({
      listen,
    }).start();
    console.log(`ADC server is running on: ${listen.origin}`);
  });
