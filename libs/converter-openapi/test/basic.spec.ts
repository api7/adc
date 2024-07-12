import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { OpenAPIV3 } from 'openapi-types';
import { parse } from 'yaml';

import { OpenAPIConverter } from '../src';
import { loadAsset, runTask } from './utils';

describe('Basic', () => {
  it('case 1 (single path)', async () => {
    const oas = parse(loadAsset('basic-1.yaml'));
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
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['POST'],
              name: 'httpbin.org_anything_post',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['DELETE'],
              name: 'httpbin.org_anything_delete',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PATCH'],
              name: 'httpbin.org_anything_patch',
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

  it('case 2 (multiple paths)', async () => {
    const oas = parse(loadAsset('basic-2.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org',
          path_prefix: '/',
          routes: [
            {
              description: 'Absolutely 302 Redirects n times.',
              methods: ['GET'],
              name: 'httpbin.org_absolute-redirectn_get',
              uris: ['/absolute-redirect/:n'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['POST'],
              name: 'httpbin.org_anything_post',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['DELETE'],
              name: 'httpbin.org_anything_delete',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PATCH'],
              name: 'httpbin.org_anything_patch',
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

  it('case 3 (multiple servers)', async () => {
    const oas = parse(loadAsset('basic-3.yaml'));
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
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['POST'],
              name: 'httpbin.org_anything_post',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['DELETE'],
              name: 'httpbin.org_anything_delete',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PATCH'],
              name: 'httpbin.org_anything_patch',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [
              { host: 'httpbin.org', port: 443, weight: 100 },
              { host: 'httpbin.net', port: 443, weight: 100 },
              { host: 'httpbin.com', port: 80, weight: 100 },
              { host: 'httpbin.com', port: 8080, weight: 100 },
              { host: 'httpbin.us', port: 80, weight: 100 }, // Path that is not the first node will be ignored
            ],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 4 (server variables)', async () => {
    const oas = parse(loadAsset('basic-4.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org',
          path_prefix: '/test1Value/test2Value', // The path prefix follows only the first server, use default values for variables
          routes: [
            {
              description: 'Returns anything passed in request data.',
              methods: ['GET'],
              name: 'httpbin.org_anything_get',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'httpbin.org_anything_put',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['POST'],
              name: 'httpbin.org_anything_post',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['DELETE'],
              name: 'httpbin.org_anything_delete',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PATCH'],
              name: 'httpbin.org_anything_patch',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [
              { host: 'httpbin.us', port: 80, weight: 100 },
              { host: 'httpbin.org', port: 443, weight: 100 },
            ],
            pass_host: 'pass',
            scheme: 'http',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 5 (servers in path/operation)', async () => {
    const oas = parse(loadAsset('basic-5.yaml'));
    const config = await runTask(new OpenAPIConverter().toADC(oas));

    expect(config).toEqual({
      services: [
        {
          description: 'httpbin.org description',
          name: 'httpbin.org',
          path_prefix: '/',
          routes: [
            {
              description: 'Absolutely 302 Redirects n times.',
              methods: ['GET'],
              name: 'httpbin.org_absolute-redirectn_get',
              uris: ['/absolute-redirect/:n'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.org', port: 443, weight: 100 }],
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
            nodes: [{ host: 'httpbin.com', port: 443, weight: 100 }],
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
            {
              description: 'Returns anything passed in request data.',
              methods: ['POST'],
              name: 'httpbin.org_anything_post',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['DELETE'],
              name: 'httpbin.org_anything_delete',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PATCH'],
              name: 'httpbin.org_anything_patch',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.net', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 6 (route-less main service)', async () => {
    const oas = parse(loadAsset('basic-6.yaml'));
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
            nodes: [{ host: 'httpbin.com', port: 443, weight: 100 }],
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
            {
              description: 'Returns anything passed in request data.',
              methods: ['POST'],
              name: 'httpbin.org_anything_post',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['DELETE'],
              name: 'httpbin.org_anything_delete',
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PATCH'],
              name: 'httpbin.org_anything_patch',
              uris: ['/anything'],
            },
          ],
          upstream: {
            nodes: [{ host: 'httpbin.net', port: 443, weight: 100 }],
            pass_host: 'pass',
            scheme: 'https',
            timeout: { connect: 60, read: 60, send: 60 },
          },
        },
      ],
    });
  });

  it('case 7 (route name)', async () => {
    const oas = parse(loadAsset('basic-7.yaml'));
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
              methods: ['GET'],
              name: 'httpbin.org_anything_get', // Use generate name
              uris: ['/anything'],
            },
            {
              description: 'Returns anything passed in request data.',
              methods: ['PUT'],
              name: 'Anything_PUT', // Use operationId if that is exist
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
});
