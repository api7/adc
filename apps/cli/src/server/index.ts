import express from 'express';
import type { Express } from 'express';
import type { Server } from 'node:http';

import { syncHandler } from './sync';

export class ADCServer {
  private readonly express: Express;
  private server?: Server;

  constructor() {
    this.express = express();
    this.express.disable('x-powered-by');
    this.express.use(express.json({ limit: '100mb' }));
    this.express.put('/sync', syncHandler);
  }

  public async start() {
    return new Promise<void>((resolve) => {
      this.server = this.express.listen(3000, '127.0.0.1', () => resolve());
    });
  }

  public TEST_ONLY_getExpress() {
    return this.express;
  }
}
