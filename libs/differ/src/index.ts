import { DifferV3 } from './differv3.js';
import { DifferV4 } from './differv4.js';

// Feature gate: set env var ENABLE_DIFFER_V4=false to opt out of DifferV4.
// Defaults to true; set ENABLE_DIFFER_V4=false to fall back to DifferV3.
export const ENABLE_DIFFER_V4 = !['false', '0'].includes(
  process.env.ENABLE_DIFFER_V4 ?? 'true',
);

export { DifferV3, DifferV4 };
export const Differ = ENABLE_DIFFER_V4 ? DifferV4 : DifferV3;
