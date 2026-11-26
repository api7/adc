import { build } from 'esbuild';
import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

describe('CLI environment variable warnings', () => {
  let testDir: string;
  let adcBinary: string;

  beforeAll(async () => {
    testDir = await mkdtemp(join(tmpdir(), 'adc-env-warning-'));
    adcBinary = join(testDir, 'adc.cjs');
    await build({
      entryPoints: [resolve(__dirname, '../src/main.ts')],
      outfile: adcBinary,
      bundle: true,
      format: 'cjs',
      platform: 'node',
    });
  });

  afterAll(async () => {
    await rm(testDir, { recursive: true, force: true });
  });

  it('warns once per source file and does not warn for an explicitly empty variable', async () => {
    const firstFile = join(testDir, 'first.yaml');
    const secondFile = join(testDir, 'second.yaml');
    await writeFile(
      firstFile,
      'services:\n  - name: service-a-${ADC_E2E_MISSING}\n',
    );
    await writeFile(
      secondFile,
      'services:\n  - name: service-b-${ADC_E2E_MISSING}-${ADC_E2E_EMPTY}\n',
    );

    const { stdout, stderr } = await execFileAsync(
      process.execPath,
      [adcBinary, 'lint', '-f', firstFile, '-f', secondFile],
      {
        env: {
          ...process.env,
          ADC_E2E_EMPTY: '',
          ADC_E2E_MISSING: undefined,
          FORCE_COLOR: '0',
        },
      },
    );
    const output = `${stdout}${stderr}`;

    expect(output.match(/ADC_E2E_MISSING/g)).toHaveLength(2);
    expect(output).toContain(firstFile);
    expect(output).toContain(secondFile);
    expect(output).not.toContain('ADC_E2E_EMPTY');
    expect(output).toContain('All is well');
  });
});
