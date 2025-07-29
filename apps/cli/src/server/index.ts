import express from 'express';
import type { Express } from 'express';
import type { Server } from 'node:http';

import { syncHandler } from './sync';

interface ADCServerOptions {
  listen: URL;
}
export class ADCServer {
  private readonly express: Express;
  private listen: URL;
  private server?: Server;

  constructor(opts: ADCServerOptions) {
    this.listen = opts.listen;
    this.express = express();
    this.express.disable('x-powered-by');
    this.express.use(express.json({ limit: '100mb' }));
    this.express.put('/sync', syncHandler);
  }

  public async start() {
    return new Promise<void>((resolve) => {
      this.server = this.express.listen(
        parseInt(this.listen.port),
        this.listen.hostname,
        () => resolve(),
      );
    });
  }

  public TEST_ONLY_getExpress() {
    return this.express;
  }
}
