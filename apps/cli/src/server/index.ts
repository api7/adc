import express from 'express';
import type { Express } from 'express';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as https from 'node:https';

import { loggerMiddleware } from './logger';
import { syncHandler } from './sync';
import { validateHandler } from './validate';

interface ADCServerOptions {
  listen: URL;
  listenStatus: number;
  tlsCert?: string;
  tlsKey?: string;
  tlsCACert?: string;
}
export class ADCServer {
  private readonly express: Express;
  private readonly expressStatus: Express;
  private server?: http.Server | https.Server;
  private serverStatus?: http.Server;

  constructor(private readonly opts: ADCServerOptions) {
    this.express = express();
    this.expressStatus = express();
    [this.express, this.expressStatus].forEach(
      (app) => (app.disable('x-powered-by'), app.disable('etag')),
    );
    this.express.use(express.json({ limit: '100mb' }));
    this.express.use(loggerMiddleware);
    this.express.put('/sync', syncHandler);
    this.express.put('/validate', validateHandler);
    this.expressStatus.get('/healthz/ready', (_, res) =>
      res.status(200).send('OK'),
    );
  }

  public async start() {
    switch (this.opts.listen.protocol) {
      case 'https:':
        this.server = https.createServer(
          {
            cert: this.opts.tlsCert,
            key: this.opts.tlsKey,
            ...(this.opts.tlsCACert
              ? {
                  ca: this.opts.tlsCACert,
                  requestCert: true,
                  rejectUnauthorized: true,
                }
              : {}),
          },
          this.express,
        );
        break;
      case 'http:':
      case 'unix:':
      default:
        this.server = http.createServer(this.express);
        break;
    }
    if (!this.server) throw new Error('Server not initialized');
    const server = this.server;
    return Promise.all([
      new Promise<void>((resolve, reject) => {
        const listen = this.opts.listen;
        const onError = (err: Error) => reject(err);
        server.once('error', onError);
        if (listen.protocol === 'unix:') {
          if (fs.existsSync(listen.pathname)) fs.unlinkSync(listen.pathname);
          server.listen(listen.pathname, () => {
            server.removeListener('error', onError);
            fs.chmodSync(listen.pathname, 0o660);
            resolve();
          });
        } else {
          const port = listen.port
            ? parseInt(listen.port, 10)
            : listen.protocol === 'https:'
              ? 443
              : 80;
          if (isNaN(port) || port < 1 || port > 65535)
            throw new Error(`Invalid listen port: "${listen.port}"`);
          server.listen(port, listen.hostname, () => {
            server.removeListener('error', onError);
            resolve();
          });
        }
      }),
      new Promise<void>((resolve, reject) => {
        const onError = (err: Error) => reject(err);
        const statusServer = this.expressStatus.listen(
          this.opts.listenStatus,
          () => {
            statusServer.removeListener('error', onError);
            resolve();
          },
        );
        this.serverStatus = statusServer;
        statusServer.once('error', onError);
      }),
    ]);
  }

  public async stop() {
    return Promise.all([
      new Promise<void>((resolve) => {
        if (this.server) {
          this.server.close(() => resolve());
        } else {
          resolve();
        }
      }),
      new Promise<void>((resolve) => {
        if (this.serverStatus) {
          this.serverStatus.close(() => resolve());
        } else {
          resolve();
        }
      }),
    ]);
  }

  public TEST_ONLY_getExpress() {
    return this.express;
  }
}
