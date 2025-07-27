import express from 'express';
import type { Express } from 'express';

import { syncHandler } from './sync';

export class ADCServer {
  private express: Express;

  constructor() {
    this.express = express();
    this.express.disable('x-powered-by');
    this.express.use(express.json({ limit: '10mb' }));
    this.express.put('/sync', syncHandler);
  }

  public start() {
    console.log('ADC server is running on: http://127.0.0.1:3000');
    this.express.listen(3000, '127.0.0.1');
  }
}
