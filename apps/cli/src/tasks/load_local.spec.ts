import * as ADCSDK from '@api7/adc-sdk';
import { dump } from 'js-yaml';
import { Listr } from 'listr2';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { LoadLocalConfigurationTask } from './load_local';

describe('LoadLocalConfigurationTask', () => {
  it('filters local resources by exact label key match', async () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'adc-load-local-'));
    const file = path.join(dir, 'adc.yaml');

    writeFileSync(
      file,
      dump({
        services: [
          {
            name: 'match-by-name',
            labels: { name: 'yanglao_wx_pgm' },
          },
          {
            name: 'should-not-match-appname',
            labels: { appname: 'yanglao_wx_pgm' },
          },
        ],
      } satisfies ADCSDK.Configuration),
      'utf8',
    );

    const ctx: { local: ADCSDK.Configuration } = { local: {} };

    try {
      await new Listr(
        [LoadLocalConfigurationTask([file], { name: 'yanglao_wx_pgm' })],
        { ctx },
      ).run();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }

    expect(ctx.local.services).toEqual([
      {
        name: 'match-by-name',
        labels: { name: 'yanglao_wx_pgm' },
      },
    ]);
  });
});
