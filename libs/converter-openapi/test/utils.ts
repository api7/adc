import { Listr, SilentRenderer } from 'listr2';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { lastValueFrom } from 'rxjs';

import { OpenAPIConverter } from '../src';

export const loadAsset = (fileName: string) =>
  readFileSync(join(__dirname, `assets/${fileName}`)).toString('utf-8');

export const runTask = async (tasks: Listr) => {
  //@ts-expect-error just ignore
  tasks.renderer = new SilentRenderer();
  await tasks.run({ local: {} });
  return tasks.ctx.local;
};

export const convert = async (oas: string) =>
  await lastValueFrom(new OpenAPIConverter().toADC(oas));
