import express from 'express';
import type { Express } from 'express';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as https from 'node:https';

import { syncHandler } from './sync';

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
    this.express.put('/sync', syncHandler);
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
    return Promise.all([
      new Promise<void>((resolve) => {
        const listen = this.opts.listen;
        if (listen.protocol === 'unix:') {
          if (fs.existsSync(listen.pathname)) fs.unlinkSync(listen.pathname);
          this.server.listen(listen.pathname, () => {
            fs.chmodSync(listen.pathname, 0o660);
            resolve();
          });
        } else {
          this.serverStatus = this.server.listen(
            parseInt(listen.port),
            listen.hostname,
            () => resolve(),
          );
        }
      }),
      new Promise<void>((resolve) => {
        this.expressStatus.listen(this.opts.listenStatus, () => resolve());
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
