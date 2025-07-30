import express from 'express';
import type { Express } from 'express';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as https from 'node:https';

import { syncHandler } from './sync';

interface ADCServerOptions {
  listen: URL;
  tlsCert?: string;
  tlsKey?: string;
  tlsCACert?: string;
}
export class ADCServer {
  private readonly opts: ADCServerOptions;
  private readonly express: Express;
  //private listen: URL;
  private server?: http.Server | https.Server;

  constructor(opts: ADCServerOptions) {
    this.opts = opts;
    this.express = express();
    this.express.disable('x-powered-by');
    this.express.use(express.json({ limit: '100mb' }));
    this.express.put('/sync', syncHandler);
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
    return new Promise<void>((resolve) => {
      const listen = this.opts.listen;
      if (listen.protocol === 'unix:') {
        if (fs.existsSync(listen.pathname)) fs.unlinkSync(listen.pathname);
        this.server.listen(listen.pathname, () => {
          fs.chmodSync(listen.pathname, 0o660);
          resolve();
        });
      } else {
        this.server.listen(parseInt(listen.port), listen.hostname, () =>
          resolve(),
        );
      }
    });
  }

  public async stop() {
    return new Promise<void>((resolve) => {
      if (this.server) {
        this.server.close(() => resolve());
      } else {
        resolve();
      }
    });
  }

  public TEST_ONLY_getExpress() {
    return this.express;
  }
}
