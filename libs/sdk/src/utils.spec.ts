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
    const getInterceptorHandler = (client: ReturnType<typeof axios.create>) => {
      const interceptor = client.interceptors.response as any;
      const handlers = interceptor.handlers;
      return handlers[handlers.length - 1].rejected;
    };

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

      try {
        await getInterceptorHandler(client)(timeoutError);
        throw new Error('Expected rejection');
      } catch (err) {
        const error = err as Error;
        const expectedMsg =
          'Request "GET https://example.com/api/gateway_groups" timed out after 5000ms. Consider increasing the timeout with the --timeout flag.';
        expect(error.message).toBe(expectedMsg);
        expect(error.stack).toContain(expectedMsg);
      }
    });

    it('should handle absolute URL without duplicating baseURL', async () => {
      const client = axios.create({ baseURL: 'https://example.com' });
      utils.registerTimeoutInterceptor(client);

      const timeoutError = new axios.AxiosError(
        'timeout of 3000ms exceeded',
        'ECONNABORTED',
        {
          method: 'put',
          url: 'https://other-server.com/api/services/123',
          baseURL: 'https://example.com',
          timeout: 3000,
          headers: new axios.AxiosHeaders(),
        },
      );

      try {
        await getInterceptorHandler(client)(timeoutError);
        throw new Error('Expected rejection');
      } catch (err) {
        const error = err as Error;
        expect(error.message).toContain(
          'https://other-server.com/api/services/123',
        );
        expect(error.message).not.toContain('https://example.com');
      }
    });

    it('should handle missing timeout value gracefully', async () => {
      const client = axios.create({ baseURL: 'https://example.com' });
      utils.registerTimeoutInterceptor(client);

      const timeoutError = new axios.AxiosError(
        'timeout exceeded',
        'ECONNABORTED',
        {
          method: 'get',
          url: '/api/version',
          baseURL: 'https://example.com',
          headers: new axios.AxiosHeaders(),
        },
      );

      try {
        await getInterceptorHandler(client)(timeoutError);
        throw new Error('Expected rejection');
      } catch (err) {
        const error = err as Error;
        expect(error.message).toContain('timed out after an unknown duration');
        expect(error.message).not.toContain('undefinedms');
      }
    });

    it('should not modify non-timeout errors', async () => {
      const client = axios.create({ baseURL: 'https://example.com' });
      utils.registerTimeoutInterceptor(client);

      const nonTimeoutError = new axios.AxiosError(
        'Request failed with status code 500',
        'ERR_BAD_RESPONSE',
      );

      await expect(getInterceptorHandler(client)(nonTimeoutError)).rejects.toThrow(
        'Request failed with status code 500',
      );
    });
  });
});
