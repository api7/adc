import type { Configuration, PluginSchemaMap } from '@api7/adc-sdk';

import { check } from '..';

const mockPluginSchemas: PluginSchemaMap = {
  'key-auth': {
    configSchema: {
      type: 'object',
      properties: {
        key: { type: 'string' },
        hide_credentials: { type: 'boolean', default: false },
      },
      required: ['key'],
    },
    consumerSchema: {
      type: 'object',
      properties: {
        key: { type: 'string' },
      },
      required: ['key'],
    },
  },
  'rate-limiting': {
    configSchema: {
      type: 'object',
      properties: {
        count: { type: 'integer', minimum: 1 },
        time_window: { type: 'integer', minimum: 1 },
        key: { type: 'string' },
        rejected_code: { type: 'integer', default: 503 },
      },
      required: ['count', 'time_window'],
    },
    metadataSchema: {
      type: 'object',
      properties: {
        limit_count_redis_cluster_name: { type: 'string' },
      },
    },
  },
  'proxy-rewrite': {
    configSchema: {
      type: 'object',
      properties: {
        uri: { type: 'string' },
        host: { type: 'string' },
        headers: {
          type: 'object',
          properties: {
            set: { type: 'object' },
            add: { type: 'object' },
            remove: { type: 'array', items: { type: 'string' } },
          },
        },
      },
    },
  },
};

describe('Plugin Linter', () => {
  describe('core validation still works without plugin schemas', () => {
    it('should pass valid config without plugin schemas', () => {
      const result = check({
        services: [
          {
            name: 'test-svc',
            routes: [{ name: 'test-route', uris: ['/test'] }],
          },
        ],
      });
      expect(result.success).toBe(true);
    });

    it('should accept any plugins without plugin schemas', () => {
      const result = check({
        services: [
          {
            name: 'test-svc',
            plugins: { 'non-existent-plugin': { foo: 'bar' } },
            routes: [{ name: 'test-route', uris: ['/test'] }],
          },
        ],
      });
      expect(result.success).toBe(true);
    });
  });

  describe('unknown plugin detection', () => {
    it('should report unknown plugin in service.plugins', () => {
      const result = check(
        {
          services: [
            {
              name: 'test-svc',
              plugins: { 'non-existent': {} },
              routes: [{ name: 'test-route', uris: ['/test'] }],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors).toEqual([
        {
          path: ['services', 0, 'plugins', 'non-existent'],
          message: 'Unknown plugin "non-existent"',
          code: 'unknown_plugin',
        },
      ]);
    });

    it('should report unknown plugin in route.plugins', () => {
      const result = check(
        {
          services: [
            {
              name: 'test-svc',
              routes: [
                {
                  name: 'test-route',
                  uris: ['/test'],
                  plugins: { 'does-not-exist': { a: 1 } },
                },
              ],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors).toContainEqual({
        path: ['services', 0, 'routes', 0, 'plugins', 'does-not-exist'],
        message: 'Unknown plugin "does-not-exist"',
        code: 'unknown_plugin',
      });
    });

    it('should report unknown plugin in global_rules', () => {
      const result = check(
        { global_rules: { 'unknown-global': {} } },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors).toContainEqual({
        path: ['global_rules', 'unknown-global'],
        message: 'Unknown plugin "unknown-global"',
        code: 'unknown_plugin',
      });
    });
  });

  describe('plugin config validation', () => {
    it('should pass valid plugin config', () => {
      const result = check(
        {
          services: [
            {
              name: 'test-svc',
              plugins: { 'key-auth': { key: 'my-key' } },
              routes: [{ name: 'test-route', uris: ['/test'] }],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(true);
    });

    it('should report missing required field', () => {
      const result = check(
        {
          services: [
            {
              name: 'test-svc',
              plugins: { 'key-auth': { hide_credentials: true } },
              routes: [{ name: 'test-route', uris: ['/test'] }],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors).toContainEqual({
        path: ['services', 0, 'plugins', 'key-auth'],
        message: expect.stringContaining('key'),
        code: 'plugin_schema_violation',
      });
    });

    it('should report type mismatch', () => {
      const result = check(
        {
          services: [
            {
              name: 'test-svc',
              plugins: {
                'rate-limiting': { count: 'not-a-number', time_window: 60 },
              },
              routes: [{ name: 'test-route', uris: ['/test'] }],
            },
          ],
        } as unknown as Configuration,
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors).toContainEqual({
        path: ['services', 0, 'plugins', 'rate-limiting', 'count'],
        message: expect.stringContaining('integer'),
        code: 'plugin_schema_violation',
      });
    });

    it('should report multiple errors at once', () => {
      const result = check(
        {
          services: [
            {
              name: 'test-svc',
              plugins: { 'rate-limiting': {} },
              routes: [{ name: 'test-route', uris: ['/test'] }],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      // Missing both 'count' and 'time_window'
      expect(result.errors.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe('traversal coverage', () => {
    it('should validate plugins in services[*].plugins', () => {
      const result = check(
        {
          services: [
            {
              name: 'svc',
              plugins: { 'key-auth': {} },
              routes: [{ name: 'r', uris: ['/'] }],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors[0].path).toEqual([
        'services',
        0,
        'plugins',
        'key-auth',
      ]);
    });

    it('should validate plugins in services[*].routes[*].plugins', () => {
      const result = check(
        {
          services: [
            {
              name: 'svc',
              routes: [
                {
                  name: 'r',
                  uris: ['/'],
                  plugins: { 'key-auth': {} },
                },
              ],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors[0].path).toEqual([
        'services',
        0,
        'routes',
        0,
        'plugins',
        'key-auth',
      ]);
    });

    it('should validate plugins in services[*].stream_routes[*].plugins', () => {
      const result = check(
        {
          services: [
            {
              name: 'svc',
              stream_routes: [
                {
                  name: 'sr',
                  plugins: { 'key-auth': {} },
                },
              ],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors[0].path).toEqual([
        'services',
        0,
        'stream_routes',
        0,
        'plugins',
        'key-auth',
      ]);
    });

    it('should validate plugins in consumers[*].plugins', () => {
      const result = check(
        {
          consumers: [
            {
              username: 'jack',
              plugins: { 'key-auth': {} },
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors[0].path).toEqual([
        'consumers',
        0,
        'plugins',
        'key-auth',
      ]);
    });

    it('should validate credentials using consumerSchema', () => {
      const result = check(
        {
          consumers: [
            {
              username: 'jack',
              credentials: [
                {
                  name: 'cred1',
                  type: 'key-auth',
                  config: {},
                },
              ],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors).toContainEqual({
        path: ['consumers', 0, 'credentials', 0, 'config', 'key-auth'],
        message: expect.stringContaining('key'),
        code: 'plugin_schema_violation',
      });
    });

    it('should validate consumer_groups[*].plugins', () => {
      const result = check(
        {
          consumer_groups: [
            {
              name: 'group1',
              plugins: { 'rate-limiting': {} },
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors[0].path[0]).toBe('consumer_groups');
    });

    it('should validate global_rules', () => {
      const result = check(
        { global_rules: { 'rate-limiting': {} } },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors[0].path[0]).toBe('global_rules');
    });

    it('should validate plugin_metadata using metadataSchema', () => {
      const result = check(
        {
          plugin_metadata: {
            'rate-limiting': { limit_count_redis_cluster_name: 123 },
          },
        } as unknown as Configuration,
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      expect(result.errors[0].path[0]).toBe('plugin_metadata');
    });
  });

  describe('edge cases', () => {
    it('should pass with empty plugins object', () => {
      const result = check(
        {
          services: [
            {
              name: 'svc',
              plugins: {},
              routes: [{ name: 'r', uris: ['/'] }],
            },
          ],
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(true);
    });

    it('should pass with valid config and plugin schemas', () => {
      const result = check(
        {
          services: [
            {
              name: 'svc',
              plugins: { 'proxy-rewrite': { uri: '/new' } },
              routes: [
                {
                  name: 'r',
                  uris: ['/'],
                  plugins: { 'key-auth': { key: 'my-key' } },
                },
              ],
            },
          ],
          global_rules: {
            'rate-limiting': { count: 10, time_window: 60 },
          },
        },
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(true);
    });

    it('should still do core validation first', () => {
      // Invalid core config (missing required 'uris' in route) should fail
      // before plugin validation
      const result = check(
        {
          services: [
            {
              name: 'svc',
              routes: [
                {
                  name: 'r',
                  // missing uris
                  plugins: { 'non-existent': {} },
                },
              ],
            },
          ],
        } as unknown as Configuration,
        { pluginSchemas: mockPluginSchemas },
      );
      expect(result.success).toBe(false);
      // Should be a core validation error, not a plugin error
      expect(
        result.errors.some((e) => e.code === 'unknown_plugin'),
      ).toBe(false);
    });
  });
});
