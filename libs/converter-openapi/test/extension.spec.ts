import { load } from 'js-yaml';

import { OpenAPIConverter } from '../src';
import { loadAsset, runTask } from './utils';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const parse = (content: string): any => load(content);

describe('Extension', () => {
  it('case 1 (override resource name)', async () => {
    const oas = parse(loadAsset('extension-1.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'override service name',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'override route name',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 2 (override resource name by empty string)', async () => {
    const oas = parse(loadAsset('extension-2.yaml'));
    await expect(runTask(new OpenAPIConverter().toADC(oas))).rejects.toThrow();
  });

  it('case 3 (add labels)', async () => {
    const oas = parse(loadAsset('extension-3.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          labels: { test1: 'test1' },
          name: 'httpbin.org',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              labels: { test2: 'test2', test3: ['test3', 'test4'] },
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 4 (incorrect labels format)', async () => {
    const oas = parse(loadAsset('extension-4.yaml'));
    await expect(runTask(new OpenAPIConverter().toADC(oas))).rejects.toThrow();
  });

  it('case 5 (add plugins)', async () => {
    const oas = parse(loadAsset('extension-5.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org',
          path_prefix: '/',
          plugins: {
            test1: { 'test1-key': 'test1-value' },
            test2: { 'test2-key': 'test3-value-override' }, // Individual plugins will override the plugin configuration in the list
            test3: { 'test3-key': 'test3-value' },
          },
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              plugins: {
                test1: { 'test1-key': 'test1-value' },
                test2: { 'test2-key': 'test3-value-override' }, // Dito
                test3: { 'test3-key': 'test3-value' },
              },
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 6 (add plugins at all levels)', async () => {
    const oas = parse(loadAsset('extension-6.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org',
          path_prefix: '/',
          plugins: {
            root1: { 'root1-key': 'value' },
            root2: { 'root2-key': 'value' },
          },
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              plugins: {
                method1: { 'method1-key': 'value' },
                method2: { 'method2-key': 'value' },
                path1: { 'path1-key': 'value' },
                path2: { 'path2-key': 'value-override' },
              },
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              plugins: {
                path1: { 'path1-key': 'value' },
                path2: { 'path2-key': 'value' },
              },
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 7 (route defaults)', async () => {
    const oas = parse(loadAsset('extension-7.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              test1: 'test1',
              test2: 'test2-override', // Override by path level x-adc-route-defaults
              test3: 'test3', // Keep due to no specific x-adc-route-defaults in the PUT method
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [
              {
                host: 'httpbin.org',
                port: 443,
                weight: 100,
              },
            ],
            pass_host: 'pass',
            scheme: 'https',
            timeout: {
              connect: 60,
              read: 60,
              send: 60,
            },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_get',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              test1: 'test1',
              test2: 'test2-override', // Override by path level x-adc-route-defaults
              test3: 'test3-override', // Override by opertaion level x-adc-route-defaults
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [
              {
                host: 'httpbin.org',
                port: 443,
                weight: 100,
              },
            ],
            pass_host: 'pass',
            scheme: 'https',
            timeout: {
              connect: 60,
              read: 60,
              send: 60,
            },
          },
        },
      ],
    });
  });

  it('case 8 (service defaults)', async () => {
    const oas = parse(loadAsset('extension-8.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_get',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3-override',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 9 (upstream defaults)', async () => {
    const oas = parse(loadAsset('extension-9.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_get',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3-override',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 10 (service and upstream defaults)', async () => {
    const oas = parse(loadAsset('extension-10.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_get',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3-override',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3-override',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 11 (route, service and upstream defaults)', async () => {
    const oas = parse(loadAsset('extension-11.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_get',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              test1: 'test1',
              test2: 'test2-override',
              test3: 'test3-override',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3-override',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3-override',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_post',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['POST'],
              name: 'httpbin.org_anything_post',
              test1: 'test1',
              test2: 'test2-override',
              test3: 'test3',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3-override',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_delete',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['DELETE'],
              name: 'httpbin.org_anything_delete',
              test1: 'test1',
              test2: 'test2-override',
              test3: 'test3',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3-override',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_patch',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['PATCH'],
              name: 'httpbin.org_anything_patch',
              test1: 'test1',
              test2: 'test2-override',
              test3: 'test3',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3-override',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3-override',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              test1: 'test1',
              test2: 'test2-override',
              test3: 'test3-override',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['OPTIONS'],
              name: 'httpbin.org_anything_options',
              test1: 'test1',
              test2: 'test2-override',
              test3: 'test3',
              uris: ['/anything'],
            },
          ],
          test1: 'test1',
          test2: 'test2-override',
          test3: 'test3',
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            test1: 'test1',
            test2: 'test2-override',
            test3: 'test3',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 12 (upstream node defaults)', async () => {
    const oas = parse(loadAsset('extension-12.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [
              {
                host: 'httpbin.org',
                port: 443,
                test1: 'test1',
                test2: 'test2',
                test3: 'test3',
                weight: 100,
              },
              { host: 'httpbin.com', port: 443, test4: 'test4', weight: 100 },
            ],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
        {
          description: 'httpbin.org description',
          name: 'httpbin.org_anything_get',
          path_prefix: '/',
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [
              {
                host: 'httpbin.org',
                port: 443,
                test1: 'test1-override',
                test2: 'test2-override',
                test3: 'test3-override',
                weight: 100,
              },
            ],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });
});
