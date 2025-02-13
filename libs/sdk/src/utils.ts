import { isUndefined, mapValues, pickBy } from 'lodash';
import { createHash } from 'node:crypto';
import { Signale } from 'signale';

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

const getLogger = (scope: Array<string> = ['ADC']) => Logger.getLogger(scope);

class Logger {
  private static instance: Signale;
  public static getLogger(scope: Array<string> = ['ADC']): Signale {
    if (!Logger.instance)
      Logger.instance = new Signale({
        config: {
          displayTimestamp: true,
        },
      });

    return new Proxy(Logger.instance.scope(...scope), {
      get: (obj, prop) => {
        /* if (prop === 'start')
          return () => console.log('NO DEBUG MODE, IGNORE LOG'); */

        return obj[prop];
      },
    });
  }
}

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
  getLogger,
  featureGate,
  featureGateEnabled,
};
