import axios, { type AxiosInstance } from 'axios';
import { isUndefined, mapValues, pickBy } from 'lodash-es';
import { createHash } from 'node:crypto';

export { Logger, LogEntry, LogEntryOptions } from './utils/logger';

const generateId = (name: string): string => {
  return createHash('sha1').update(name).digest('hex');
};

const recursiveOmitUndefined = <T extends object>(obj0: T): T => {
  const obj = Object.isFrozen(obj0) ? structuredClone(obj0) : obj0;
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

const registerTimeoutInterceptor = (client: AxiosInstance) => {
  client.interceptors.response.use(undefined, (error) => {
    if (axios.isAxiosError(error) && error.code === 'ECONNABORTED') {
      const method = error.config?.method?.toUpperCase() ?? 'UNKNOWN';
      const rawUrl = error.config?.url ?? '';
      const url = /^https?:\/\//i.test(rawUrl)
        ? rawUrl
        : `${error.config?.baseURL ?? ''}${rawUrl}`;
      const timeout = error.config?.timeout;
      const timeoutText =
        typeof timeout === 'number' ? `${timeout}ms` : 'an unknown duration';
      const newMessage = `Request "${method} ${url}" timed out after ${timeoutText}. Consider increasing the timeout with the --timeout flag.`;
      error.message = newMessage;
      if (error.stack) {
        error.stack = error.stack.replace(
          /^(.*?):\s*(.*)/,
          `$1: ${newMessage}`,
        );
      }
    }
    return Promise.reject(error);
  });
};

export const utils = {
  generateId,
  recursiveOmitUndefined,
  featureGate,
  featureGateEnabled,
  registerTimeoutInterceptor,
};
