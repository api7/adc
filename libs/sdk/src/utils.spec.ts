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

  describe('formatAxiosErrorMessage', () => {
    it('should format error with error_msg from response data', () => {
      const error = {
        config: {
          method: 'put',
          url: '/apisix/admin/routes/123',
          baseURL: 'https://example.com',
        },
        response: {
          status: 400,
          statusText: 'Bad Request',
          data: { error_msg: 'route name is reduplicate' },
        },
      };
      const msg = utils.formatAxiosErrorMessage(error);
      expect(msg).toBe(
        'PUT https://example.com/apisix/admin/routes/123, responded with status 400 Bad Request, error_msg: route name is reduplicate',
      );
    });

    it('should format error when response body is empty string (Error: "" scenario)', () => {
      const error = {
        config: {
          method: 'put',
          url: '/apisix/admin/routes/456',
          baseURL: 'https://dashboard.example.com',
        },
        response: {
          status: 500,
          statusText: 'Internal Server Error',
          data: '',
        },
      };
      const msg = utils.formatAxiosErrorMessage(error);
      expect(msg).toBe(
        'PUT https://dashboard.example.com/apisix/admin/routes/456, responded with status 500 Internal Server Error',
      );
    });

    it('should include response body when no error_msg field exists', () => {
      const error = {
        config: {
          method: 'get',
          url: '/api/services',
          baseURL: 'https://example.com',
        },
        response: {
          status: 502,
          statusText: 'Bad Gateway',
          data: '<html><body>Bad Gateway</body></html>',
        },
      };
      const msg = utils.formatAxiosErrorMessage(error);
      expect(msg).toContain('responded with status 502 Bad Gateway');
      expect(msg).toContain(
        'response body: <html><body>Bad Gateway</body></html>',
      );
    });

    it('should handle absolute URL without duplicating baseURL', () => {
      const error = {
        config: {
          method: 'delete',
          url: 'https://other.com/api/routes/789',
          baseURL: 'https://example.com',
        },
        response: {
          status: 404,
          statusText: 'Not Found',
          data: { error_msg: 'not found' },
        },
      };
      const msg = utils.formatAxiosErrorMessage(error);
      expect(msg).toContain('DELETE https://other.com/api/routes/789');
      expect(msg).not.toContain('https://example.com');
    });

    it('should handle missing config gracefully', () => {
      const error = {
        response: {
          status: 500,
          statusText: 'Internal Server Error',
          data: null,
        },
      };
      const msg = utils.formatAxiosErrorMessage(error);
      expect(msg).toContain('UNKNOWN');
      expect(msg).toContain('responded with status 500');
    });

    it('should handle JSON object response without error_msg', () => {
      const error = {
        config: {
          method: 'post',
          url: '/api/consumers',
          baseURL: 'https://example.com',
        },
        response: {
          status: 409,
          statusText: 'Conflict',
          data: { code: 10001, message: 'conflict detected' },
        },
      };
      const msg = utils.formatAxiosErrorMessage(error);
      expect(msg).toContain('responded with status 409 Conflict');
      expect(msg).toContain(
        'response body: {"code":10001,"message":"conflict detected"}',
      );
    });
  });
});
