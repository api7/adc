import { isUndefined, mapValues, pickBy } from 'lodash';
import { createHash } from 'node:crypto';

const generateId = (name: string): string => {
  return createHash('sha1').update(name).digest('hex');
};

const recursiveOmitUndefined = <T extends object>(obj: T) => {
  return typeof obj === 'object' && !Array.isArray(obj)
    ? (pickBy(
        mapValues(obj, recursiveOmitUndefined),
        (value) => !isUndefined(value),
      ) as T)
    : obj;
};

enum featureGate {
  REMOTE_STATE_FILE = 'remote-state-file',
  PARALLEL_BACKEND_REQUEST = 'parallel-backend-request',
}
const featureGateEnabled = (key: string) => {
  return (
    process.env.ADC_EXPERIMENTAL_FEATURE_FLAGS &&
    process.env.ADC_EXPERIMENTAL_FEATURE_FLAGS.includes(key)
  );
};

export const utils = {
  generateId,
  recursiveOmitUndefined,
  featureGate,
  featureGateEnabled,
};
