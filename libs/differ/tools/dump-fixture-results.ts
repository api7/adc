// Runs `DifferV4.diff()` over every fixture in `fixtures/differ/*.json` and
// dumps the results to a JSON file, so `scripts/compare-differ-fixtures.mjs`
// can compare them against the Rust side's `run_fixtures` binary output.
//
// Not a real test (no assertions) — vitest is only used here because this
// repo's TS enums (ADCSDK.EventType/ResourceType) need real transpilation,
// which Node's native TS support (type-stripping only) can't do.
//
// Env vars:
//   ADC_DIFFER_FIXTURES_DIR       fixtures directory (default: <repo root>/fixtures/differ)
//   ADC_DIFFER_FIXTURE_RESULTS_OUT  output file path (default: /tmp/adc-ts-differ-fixture-results.json)

import { basename, dirname, extname, join } from 'node:path';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { it } from 'vitest';

import { DifferV4 } from '../src/differv4.js';

// libs/differ/tools/dump-fixture-results.ts -> repo root is three levels up.
const REPO_ROOT = dirname(dirname(dirname(dirname(fileURLToPath(import.meta.url)))));

const FIXTURES_DIR = process.env.ADC_DIFFER_FIXTURES_DIR ?? join(REPO_ROOT, 'fixtures/differ');
const OUT_FILE =
  process.env.ADC_DIFFER_FIXTURE_RESULTS_OUT ?? '/tmp/adc-ts-differ-fixture-results.json';

it('dumps DifferV4.diff() output for every fixture to a JSON file', () => {
  const results: Record<string, unknown> = {};

  const files = readdirSync(FIXTURES_DIR)
    .filter((f) => extname(f) === '.json')
    .sort();

  for (const file of files) {
    const name = basename(file, '.json');
    let fixture: { local?: unknown; remote?: unknown; defaultValue?: unknown };
    try {
      fixture = JSON.parse(readFileSync(join(FIXTURES_DIR, file), 'utf-8'));
    } catch (e) {
      throw new Error(`failed to read/parse fixture ${file}: ${e}`, { cause: e });
    }
    results[name] = DifferV4.diff(fixture.local ?? {}, fixture.remote ?? {}, fixture.defaultValue);
  }

  writeFileSync(OUT_FILE, JSON.stringify(results, null, 2));
});
