import * as ADCSDK from '@api7/adc-sdk';
import { produce } from 'immer';

import * as typing from './typing';

export const toADC = (config: typing.APISIXStandaloneType) =>
  typing.APISIXStandaloneWithConfVersion.transform<ADCSDK.Configuration>(
    (input: typing.APISIXStandaloneType) => {
      const consumerCredentials = input.consumers?.filter(
        (consumerOrConsumerCredential) =>
          'name' in consumerOrConsumerCredential,
      );
      return {
        services:
          input.services
            ?.map((service) =>
              produce<ADCSDK.Service>({} as ADCSDK.Service, (draft) => {
                draft.id = service.id;
                draft.name = service.name;
                draft.description = service.desc;
                draft.labels = service.labels;
                draft.upstream = service.upstream;
                draft.plugins = service.plugins;
                draft.hosts = service.hosts;
                draft.routes = input.routes
                  ?.filter((route) => route.service_id === service.id)
                  .map((route) =>
                    produce({} as ADCSDK.Route, (draft) => {
                      draft.id = route.id;
                      draft.name = route.name;
                      draft.description = route.desc;
                      draft.labels = route.labels;
                      draft.uris = route.uris;
                      draft.hosts = route.hosts;
                      draft.priority = route.priority;
                      draft.timeout = route.timeout;
                      draft.vars = route.vars;
                      draft.methods = route.methods;
                      draft.enable_websocket = route.enable_websocket;
                      draft.remote_addrs = route.remote_addrs;
                      draft.plugins = route.plugins;
                      draft.filter_func = route.filter_func;
                    }),
                  )
                  .map(ADCSDK.utils.recursiveOmitUndefined);
                draft.stream_routes = input.stream_routes
                  ?.filter((route) => route.service_id === service.id)
                  .map((route) =>
                    produce({} as ADCSDK.StreamRoute, (draft) => {
                      draft.id = route.id;
                      draft.name = route.name;
                      draft.description = route.desc;
                      draft.labels = route.labels;
                      draft.remote_addr = route.remote_addr;
                      draft.server_addr = route.server_addr;
                      draft.server_port = route.server_port;
                      draft.sni = route.sni;
                      draft.plugins = route.plugins;
                    }),
                  )
                  .map(ADCSDK.utils.recursiveOmitUndefined);
                draft.upstreams = input.upstreams
                  ?.filter(
                    (upstream) =>
                      upstream.labels[typing.ADC_UPSTREAM_SERVICE_ID_LABEL] ===
                      service.id,
                  )
                  .map((upstream) =>
                    produce({} as ADCSDK.Upstream, (draft) => {
                      draft.id = upstream.id;
                      draft.name = upstream.name;
                      draft.description = upstream.desc;
                      draft.labels = upstream.labels;
                      draft.type = upstream.type;
                      draft.hash_on = upstream.hash_on;
                      draft.key = upstream.key;
                      draft.nodes = upstream.nodes;
                      draft.scheme = upstream.scheme;
                      draft.retries = upstream.retries;
                      draft.retry_timeout = upstream.retry_timeout;
                      draft.timeout = upstream.timeout;
                      draft.tls = upstream.tls;
                      draft.keepalive_pool = upstream.keepalive_pool;
                      draft.pass_host = upstream.pass_host;
                      draft.upstream_host = upstream.upstream_host;
                    }),
                  )
                  .map(ADCSDK.utils.recursiveOmitUndefined);

                // fill in placeholder for path prefix
                // these properties are not implemented on APISIX, so they must be
                // populated to cope with the local defaults populated by zod
                if (draft.routes?.length > 0) {
                  draft.path_prefix = '/';
                  draft.strip_path_prefix = true;
                }
              }),
            )
            .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
        ssls:
          input.ssls
            ?.map((ssl) =>
              produce<ADCSDK.SSL>({} as ADCSDK.SSL, (draft) => {
                draft.id = ssl.id;
                draft.labels = ssl.labels;
                draft.type = ssl.type;
                draft.snis = ssl.snis;
                draft.certificates = [
                  {
                    certificate: ssl.cert,
                    key: ssl.key,
                  },
                  ...(ssl.certs ?? []).map((cert, idx) => ({
                    certificate: cert,
                    key: ssl.keys[idx],
                  })),
                ] as Array<ADCSDK.SSLCertificate>;
                draft.client = ssl.client;
                draft.ssl_protocols = ssl.ssl_protocols;
              }),
            )
            .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
        consumers:
          input.consumers
            ?.filter(
              (consumerOrConsumerCredential) =>
                'username' in consumerOrConsumerCredential,
            )
            .map((consumer) =>
              produce<ADCSDK.Consumer>({} as ADCSDK.Consumer, (draft) => {
                draft.username = consumer.username;
                draft.description = consumer.desc;
                draft.labels = consumer.labels;
                draft.plugins = consumer.plugins;
                draft.credentials = consumerCredentials
                  ?.filter((credential) =>
                    credential.id.startsWith(
                      `${consumer.username}/credentials/`,
                    ),
                  )
                  .map(
                    (credential) =>
                      produce<ADCSDK.ConsumerCredential>(
                        {} as ADCSDK.ConsumerCredential,
                        (draft) => {
                          draft.id = credential.id;
                          draft.name = credential.name;
                          draft.description = credential.desc;
                          draft.labels = credential.labels;
                          const plugin = Object.entries(credential.plugins)[0];
                          draft.type =
                            plugin[0] as ADCSDK.ConsumerCredential['type'];
                          draft.config = plugin[1];
                        },
                      ) as ADCSDK.ConsumerCredential,
                  )
                  .map(ADCSDK.utils.recursiveOmitUndefined);
              }),
            )
            .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
        global_rules: Object.fromEntries(
          input.global_rules
            ?.map((globalRule) => [globalRule.id, globalRule.plugins[0]])
            .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
        ),
        plugin_metadata: Object.fromEntries(
          input.plugin_metadata
            ?.map((pluginMetadata) => {
              const { id, ...rest } = pluginMetadata;
              return [id, rest];
            })
            .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
        ),
      } satisfies ADCSDK.Configuration as ADCSDK.Configuration;
    },
  ).parse(config);
