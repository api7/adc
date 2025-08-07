import chalk from 'chalk';
import commander, { Option } from 'commander';
import { readFileSync } from 'node:fs';

import { ADCServer } from '../server';
import { BaseCommand, BaseOptions, processCertificateFile } from './helper';

type IngressServerOptions = {
  listen?: URL;
  listenStatus?: number;
  caCertFile?: string;
  tlsCertFile?: string;
  tlsKeyFile?: string;
} & BaseOptions;

export const IngressServerCommand = new BaseCommand<IngressServerOptions>(
  'server',
)
  .option<URL>(
    '--listen <string>',
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
  .option<number>(
    '--listen-status <number>',
    'status listen port',
    (val) => {
      const port = parseInt(val, 10);
      if (!port || isNaN(port) || port < 1 || port > 65535)
        throw new commander.InvalidArgumentError(
          'The status listen port must be a number between 1 and 65535',
        );
      return port;
    },
    3001,
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
  .handle(
    async ({ listen, listenStatus, tlsCertFile, tlsKeyFile, caCertFile }) => {
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
        listenStatus,
        tlsCert: tlsCertFile ? readFileSync(tlsCertFile, 'utf-8') : undefined,
        tlsKey: tlsKeyFile ? readFileSync(tlsKeyFile, 'utf-8') : undefined,
        tlsCACert: caCertFile ? readFileSync(caCertFile, 'utf-8') : undefined,
      });
      await server.start();
      console.log(
        `ADC server is running on: ${listen.protocol === 'unix:' ? listen.pathname : listen.origin}`,
      );
      process.on('SIGINT', () => {
        console.log('Stopping, see you next time!');
        server.stop();
        process.exit(0);
      });
    },
  );
