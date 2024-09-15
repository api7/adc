import * as ADCSDK from '@api7/adc-sdk';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { BackendAPI7 } from '../src';
import { dumpConfiguration, syncEvents } from './support/utils';

describe('Sync and Dump - 2', () => {
  let backend: BackendAPI7;

  beforeAll(() => {
    backend = new BackendAPI7({
      server: process.env.SERVER,
      token: process.env.TOKEN,
      tlsSkipVerify: true,
      gatewayGroup: 'default',
    });
  });

  describe('Sync and dump mixed configuration', () => {
    const testData: Array<ADCSDK.Event> = JSON.parse(
      readFileSync(join(__dirname, 'assets/testdata/mixed-1.json')).toString(
        'utf-8',
      ),
    );
    const testDataClean: Array<ADCSDK.Event> = JSON.parse(
      readFileSync(
        join(__dirname, 'assets/testdata/mixed-1-clean.json'),
      ).toString('utf-8'),
    );
    let dump: ADCSDK.Configuration;

    it('Sync', async () => syncEvents(backend, testData));

    it('Dump', async () => {
      dump = await dumpConfiguration(backend);
    });

    it('Check', () => {
      expect(dump.ssls[0]).toMatchObject({
        type: 'server',
        snis: ['test.com'],
        certificates: [
          {
            certificate:
              '-----BEGIN CERTIFICATE-----\nMIICrTCCAZUCFCcH5+jEDUhpTxEQo/pZYC91e2aYMA0GCSqGSIb3DQEBCwUAMBEx\nDzANBgNVBAMMBlJPT1RDQTAgFw0yNDAxMTgwNjAzMDNaGA8yMTIzMTIyNTA2MDMw\nM1owEzERMA8GA1UEAwwIdGVzdC5jb20wggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAw\nggEKAoIBAQCVkfufMRK2bckdpQ/aRfcaTxmjsv5Mb+sJdhb0QuEuXp/VgN3yzFM0\nzCmAeBZwNKpU3HZDv0tnkTx7OARYpj5Bw1ole0EfPVPKBRjlLE56tabzyd4vdLV2\nbk7jYH+H8NjGZNEYLm9MdWiB4Ulyc0+XFA0ZL5WWKOi+oSQVUibT8QK0CENFKNLP\nQjEXlbyujzRS3u6r99EEEy8+3psBA2EELq8GAjEp+jilWggBhUEpLQxCHhHeNevR\nkg5iEvhOhEVKtr5xvgolg5Wvz7GmDulIW9MCu0dIXim52H/spPwgi3yRraY1XjxU\nREyj5tcY7n7LBESkx/ODXEyCkICIPpo9AgMBAAEwDQYJKoZIhvcNAQELBQADggEB\nADBU5XvbnjaF4rpQoqdzgK6BuRvD/Ih/rh+xc+G9mm+qaHx0g3TdTqyhCvSg6aRW\njDq4Z0NILdb6wmJcunua1jjmOQMXER5y34Xfn21+dzjLN2Bl+/vZ/HyXlCjxkppG\nZAsd1H0/jmXqN1zddIThxOccmRcDEP+9GT3hba50sijFbO30Zx+ORJCoT8he6Kyw\nKdOs/yyukafoAtlpoPR+ao/kumto6w/rLfFlEsehU0dMGNgPVSxxVNtBSdxPTUBk\nD6mfqB4f//2DuAmiO+l5RmPUmumqzcYlpd+oAdy3OSnNEHbgxishZr/GI3s6DmUh\n16bgI69aQ5F+MnN3trvaufc=\n-----END CERTIFICATE-----\n',
          },
        ],
      });

      dump.services = dump.services.sort((a, b) =>
        a.name.localeCompare(b.name),
      );
      expect(dump.services[0]).toMatchObject({
        name: 'service1',
        description: 'service1 description',
        upstream: {
          name: 'default',
          scheme: 'http',
          type: 'roundrobin',
          hash_on: 'vars',
          nodes: [
            {
              host: 'host',
              port: 1100,
              weight: 1100,
              priority: 0,
            },
          ],
          retry_timeout: 0,
          pass_host: 'pass',
          checks: {
            active: {
              type: 'tcp',
              timeout: 1,
              concurrency: 10,
              http_path: '/',
              https_verify_certificate: true,
              healthy: {
                interval: 1,
                http_statuses: [200, 302],
                successes: 2,
              },
              unhealthy: {
                interval: 1,
                http_statuses: [429, 404, 500, 501, 502, 503, 504, 505],
                http_failures: 5,
                tcp_failures: 2,
                timeouts: 3,
              },
            },
          },
        },
        plugins: {
          'limit-count': {
            allow_degradation: false,
            count: 2,
            key: '$consumer_name $remote_addr',
            key_type: 'var_combination',
            policy: 'local',
            rejected_code: 503,
            show_limit_quota_header: true,
            time_window: 60,
          },
        },
      });
      expect(dump.services[0].routes[0]).toMatchObject({
        uris: ['/anything'],
        name: 'route1.1',
        methods: ['GET'],
        enable_websocket: false,
        plugins: {
          'limit-count': {
            allow_degradation: false,
            count: 2,
            key: '$consumer_name $remote_addr',
            key_type: 'var_combination',
            policy: 'local',
            rejected_code: 503,
            show_limit_quota_header: true,
            time_window: 60,
          },
        },
      });
      expect(dump.services[0].routes[1]).toMatchObject({
        uris: ['/anything'],
        name: 'route1.2',
        methods: ['POST'],
        enable_websocket: false,
      });

      expect(dump.services[1]).toMatchObject({
        name: 'service2',
        description: 'service2 description',
        upstream: {
          name: 'default',
          scheme: 'http',
          type: 'roundrobin',
          hash_on: 'vars',
          nodes: [
            {
              host: 'host',
              port: 1100,
              weight: 1100,
              priority: 0,
            },
          ],
          retry_timeout: 0,
          pass_host: 'pass',
        },
      });
      expect(dump.services[1].routes[0]).toMatchObject({
        uris: ['/postSomething'],
        name: 'route2.2',
        methods: ['POST', 'PUT'],
        enable_websocket: false,
      });
      expect(dump.services[1].routes[1]).toMatchObject({
        uris: ['/getSomething'],
        name: 'route2.1',
        methods: ['GET', 'POST'],
        enable_websocket: false,
        plugins: {
          'limit-count': {
            allow_degradation: false,
            count: 2,
            key: '$consumer_name $remote_addr',
            key_type: 'var_combination',
            policy: 'local',
            rejected_code: 503,
            show_limit_quota_header: true,
            time_window: 60,
          },
        },
      });

      expect(dump.consumers[0]).toMatchObject({
        username: 'tom',
        plugins: {
          'limit-count': {
            window: 1,
            count: 1,
          },
        },
      });

      expect(dump.global_rules.prometheus).toMatchObject({
        prefer_name: false,
      });

      expect(dump.plugin_metadata['http-logger']).toMatchObject({
        log_format: {
          '@timestamp': '$time_iso8601',
          client_ip: '$remote_addr',
          host: '$host',
        },
      });

      expect(dump.plugin_metadata['tcp-logger']).toMatchObject({
        log_format: {
          '@timestamp': '$time_iso8601',
          client_ip: '$remote_addr',
          host: '$host',
        },
      });
    });

    it('Cleanup', async () => syncEvents(backend, testDataClean));
  });
});
