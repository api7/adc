import axios from 'axios';

import { utils } from './utils';

describe('SDK utils', () => {
  it('recursiveOmitUndefined', () => {
    expect(
      utils.recursiveOmitUndefined({
        test: 'test',
        removed: undefined,
        test2: {
          test3: 'test',
          removed: undefined,
        },
        test5: ['test', undefined],
      }),
    ).toEqual({
      test: 'test',
      test2: { test3: 'test' },
      test5: ['test', undefined],
    });
  });

  describe('registerTimeoutInterceptor', () => {
    it('should enhance timeout error message with request details', async () => {
      const client = axios.create({
        baseURL: 'https://example.com',
        timeout: 5000,
      });
      utils.registerTimeoutInterceptor(client);

      const timeoutError = new axios.AxiosError(
        'timeout of 5000ms exceeded',
        'ECONNABORTED',
        {
          method: 'get',
          url: '/api/gateway_groups',
          baseURL: 'https://example.com',
          timeout: 5000,
          headers: new axios.AxiosHeaders(),
        },
      );

      // Simulate the interceptor by manually invoking it
      const interceptor = client.interceptors.response as any;
      const handlers = interceptor.handlers;
      const rejectedHandler = handlers[handlers.length - 1].rejected;

      await expect(rejectedHandler(timeoutError)).rejects.toThrow(
        'Request "GET https://example.com/api/gateway_groups" timed out after 5000ms. Consider increasing the timeout with the --timeout flag.',
      );
    });

    it('should not modify non-timeout errors', async () => {
      const client = axios.create({ baseURL: 'https://example.com' });
      utils.registerTimeoutInterceptor(client);

      const nonTimeoutError = new axios.AxiosError(
        'Request failed with status code 500',
        'ERR_BAD_RESPONSE',
      );

      const interceptor = client.interceptors.response as any;
      const handlers = interceptor.handlers;
      const rejectedHandler = handlers[handlers.length - 1].rejected;

      await expect(rejectedHandler(nonTimeoutError)).rejects.toThrow(
        'Request failed with status code 500',
      );
    });
  });
});
