#!/usr/bin/env node
// One-command TS/Rust differ parity check.
//
// Runs the same fixtures/differ/*.json inputs through both DifferV4
// implementations (TS via vitest, Rust via `cargo run`) and structurally
// compares their output. Exits non-zero on any mismatch.
//
// Usage: node scripts/compare-differ-fixtures.mjs

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

const REPO_ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

const FIXTURES_DIR = process.env.ADC_DIFFER_FIXTURES_DIR ?? path.join(REPO_ROOT, 'fixtures/differ');
const TS_RESULTS_OUT =
  process.env.ADC_DIFFER_FIXTURE_RESULTS_OUT ?? '/tmp/adc-ts-differ-fixture-results.json';
const RUST_RESULTS_OUT = '/tmp/adc-rust-differ-fixture-results.json';

// Only the Event envelope's key casing differs between the two engines (Rust
// serializes snake_case, TS uses camelCase). This map is intentionally
// shallow and explicit: resource bodies nested inside old_value/new_value/diff
// are raw APISIX config and legitimately contain real snake_case field names
// (time_window, server_port, ...) that must NOT be touched by normalization.
const ENVELOPE_KEY_MAP = {
  resource_type: 'resourceType',
  resource_id: 'resourceId',
  resource_name: 'resourceName',
  old_value: 'oldValue',
  new_value: 'newValue',
  parent_id: 'parentId',
};

function normalizeEvent(event) {
  const out = {};
  for (const [key, value] of Object.entries(event)) {
    out[ENVELOPE_KEY_MAP[key] ?? key] = value;
  }
  return out;
}

console.log(`[1/3] running TS DifferV4 over fixtures in ${FIXTURES_DIR}...`);
execFileSync('npx', ['nx', 'run', 'differ:dump-fixtures'], {
  cwd: REPO_ROOT,
  stdio: 'inherit',
  env: {
    ...process.env,
    ADC_DIFFER_FIXTURES_DIR: FIXTURES_DIR,
    ADC_DIFFER_FIXTURE_RESULTS_OUT: TS_RESULTS_OUT,
  },
});

console.log('[2/3] running Rust DifferV4 over the same fixtures...');
execFileSync(
  'cargo',
  ['run', '--quiet', '-p', 'adc-differ', '--bin', 'run_fixtures', '--', FIXTURES_DIR, '--out', RUST_RESULTS_OUT],
  { cwd: path.join(REPO_ROOT, 'rust'), stdio: 'inherit' },
);

console.log('[3/3] comparing...\n');
const tsResults = JSON.parse(readFileSync(TS_RESULTS_OUT, 'utf-8'));
const rustResults = JSON.parse(readFileSync(RUST_RESULTS_OUT, 'utf-8'));

const allNames = [...new Set([...Object.keys(tsResults), ...Object.keys(rustResults)])].sort();
const failures = [];
let passCount = 0;

for (const name of allNames) {
  const tsEvents = tsResults[name];
  const rustEvents = rustResults[name];

  if (tsEvents === undefined) {
    failures.push({ name, reason: 'missing from TS results' });
    continue;
  }
  if (rustEvents === undefined) {
    failures.push({ name, reason: 'missing from Rust results' });
    continue;
  }
  if (!Array.isArray(rustEvents)) {
    failures.push({ name, reason: `Rust result is not an array (got ${typeof rustEvents})` });
    continue;
  }

  const normalizedRust = rustEvents.map(normalizeEvent);
  if (isDeepStrictEqual(tsEvents, normalizedRust)) {
    passCount++;
  } else {
    failures.push({ name, reason: 'output mismatch', ts: tsEvents, rust: normalizedRust });
  }
}

console.log(`${passCount}/${allNames.length} fixtures match.\n`);

for (const f of failures) {
  console.log(`FAIL  ${f.name}  (${f.reason})`);
  if (f.ts !== undefined) {
    console.log('  --- TS ---');
    console.log(JSON.stringify(f.ts, null, 2).replace(/^/gm, '  '));
    console.log('  --- Rust (normalized) ---');
    console.log(JSON.stringify(f.rust, null, 2).replace(/^/gm, '  '));
  }
}

if (failures.length > 0) {
  process.exit(1);
}
console.log('All fixtures match.');
