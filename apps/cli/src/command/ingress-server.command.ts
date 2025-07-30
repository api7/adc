import chalk from 'chalk';
import commander, { Option } from 'commander';
import { readFileSync } from 'node:fs';

import { ADCServer } from '../server';
import { BaseCommand, BaseOptions, processCertificateFile } from './helper';

type IngressServerOptions = {
  listen?: URL;
  caCertFile?: string;
  tlsCertFile?: string;
  tlsKeyFile?: string;
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
  .addOption(
    new Option(
      '--ca-cert-file <string>',
      'path to the CA certificate to verify the client',
    ).argParser((value) =>
      processCertificateFile(
        value,
        'The specified CA certificate file does not exist',
      ),
    ),
  )
  .addOption(
    new Option(
      '--tls-cert-file <string>',
      'path to the TLS server certificate',
    ).argParser((value) =>
      processCertificateFile(
        value,
        'The specified TLS server certificate file does not exist',
      ),
    ),
  )
  .addOption(
    new Option(
      '--tls-key-file <string>',
      'path to the TLS server key',
    ).argParser((value) =>
      processCertificateFile(
        value,
        'The specified TLS server key file does not exist',
      ),
    ),
  )
  .handle(async ({ listen, tlsCertFile, tlsKeyFile, caCertFile }) => {
    if (listen.protocol === 'https:' && (!tlsCertFile || !tlsKeyFile)) {
      console.error(
        chalk.red(
          'Error: When using HTTPS, both --tls-cert-file and --tls-key-file must be provided',
        ),
      );
      return;
    }

    const server = new ADCServer({
      listen,
      tlsCert: readFileSync(tlsCertFile, 'utf-8'),
      tlsKey: readFileSync(tlsKeyFile, 'utf-8'),
      tlsCACert: caCertFile ? readFileSync(caCertFile, 'utf-8') : undefined,
    });
    await server.start();
    console.log(
      `ADC server is running on: ${listen.protocol === 'unix:' ? listen.pathname : listen.origin}`,
    );
    process.on('SIGINT', () => {
      console.log('Stopping, see you next time!');
      server.stop();
    });
  });
