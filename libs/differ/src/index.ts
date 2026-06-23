import { DifferV3 } from './differv3.js';
import { DifferV4 } from './differv4.js';

// Feature gate: set env var ENABLE_DIFFER_V4=true to opt in to DifferV4.
// Defaults to false in this patch release; will default to true in the next minor.
export const ENABLE_DIFFER_V4 = ['true', '1'].includes(
  process.env.ENABLE_DIFFER_V4 ?? '',
);

export { DifferV3, DifferV4 };
export const Differ = ENABLE_DIFFER_V4 ? DifferV4 : DifferV3;
