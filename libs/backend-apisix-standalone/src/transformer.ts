import * as ADCSDK from '@api7/adc-sdk';
import { isEmpty } from 'lodash';

import * as typing from './typing';

export const toADC = (input: typing.APISIXStandalone) => {
  const consumerCredentials = input.consumers?.filter(
    (consumerOrConsumerCredential) => 'name' in consumerOrConsumerCredential,
  );

  const transformUpstream = (
    upstream: Omit<typing.Upstream, 'id' | 'name' | 'modifiedIndex'> & {
      name?: string;
    },
  ) => ({
    name: upstream.name,
    description: upstream.desc,
    labels: upstream.labels,
    type: upstream.type,
    hash_on: upstream.hash_on,
    key: upstream.key,
    scheme: upstream.scheme,
    retries: upstream.retries,
    retry_timeout: upstream.retry_timeout,
    timeout: upstream.timeout,
    tls: upstream.tls,
    keepalive_pool: upstream.keepalive_pool,
    pass_host: upstream.pass_host,
    upstream_host: upstream.upstream_host,

    // Empty Lua tables will be encoded as "{}" rather than "[]" by cjson,
    // so this must be handled separately to prevent unexpected diff results.
    nodes: !isEmpty(upstream.nodes) ? upstream.nodes : [],
  });
  return {
    services:
      input.services
        ?.map((service) => ({
          id: service.id,
          name: service.name,
          description: service.desc,
          labels: service.labels,
          upstream: ADCSDK.utils.recursiveOmitUndefined(
            transformUpstream(service.upstream!),
          ),
          plugins: service.plugins,
          hosts: service.hosts,
          routes: input.routes
            ?.filter((route) => route.service_id === service.id)
            .map((route) => ({
              id: route.id,
              name: route.name,
              description: route.desc,
              labels: route.labels,
              uris: route.uris,
              hosts: route.hosts,
              priority: route.priority,
              timeout: route.timeout,
              vars: route.vars,
              methods: route.methods,
              enable_websocket: route.enable_websocket,
              remote_addrs: route.remote_addrs,
              plugins: route.plugins,
              filter_func: route.filter_func,
            }))
            .map(ADCSDK.utils.recursiveOmitUndefined),
          stream_routes: input.stream_routes
            ?.filter((route) => route.service_id === service.id)
            .map((route) => ({
              id: route.id,
              name: route.name,
              description: route.desc,
              labels: route.labels,
              remote_addr: route.remote_addr,
              server_addr: route.server_addr,
              server_port: route.server_port,
              sni: route.sni,
              plugins: route.plugins,
            }))
            .map(ADCSDK.utils.recursiveOmitUndefined),
          upstreams: input.upstreams
            ?.filter(
              (upstream) =>
                upstream.labels![typing.ADC_UPSTREAM_SERVICE_ID_LABEL] ===
                service.id,
            )
            .map(transformUpstream)
            .map(ADCSDK.utils.recursiveOmitUndefined),
        }))
        .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
    ssls:
      input.ssls
        ?.map((ssl) => ({
          id: ssl.id,
          labels: ssl.labels,
          type: ssl.type,
          snis: ssl.snis!,
          certificates: [
            {
              certificate: ssl.cert,
              key: ssl.key,
            },
            ...(ssl.certs ?? []).map((cert, idx) => ({
              certificate: cert,
              key: ssl.keys![idx],
            })),
          ] as Array<ADCSDK.SSLCertificate>,
          client: ssl.client,
          ssl_protocols: ssl.ssl_protocols,
        }))
        .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
    consumers:
      input.consumers
        ?.filter(
          (consumerOrConsumerCredential) =>
            'username' in consumerOrConsumerCredential,
        )
        .map((consumer) => ({
          username: consumer.username,
          description: consumer.desc,
          labels: consumer.labels,
          plugins: consumer.plugins,
          credentials: consumerCredentials
            ?.filter((credential) =>
              credential.id.startsWith(`${consumer.username}/credentials/`),
            )
            .map((credential) => {
              const plugin = Object.entries(credential.plugins!)[0];
              return {
                id: credential.id.replace(
                  `${consumer.username}/credentials/`,
                  '',
                ),
                name: credential.name,
                description: credential.desc,
                labels: credential.labels,

                type: plugin[0] as ADCSDK.ConsumerCredential['type'],
                config: plugin[1],
              } as ADCSDK.ConsumerCredential;
            })
            .map(ADCSDK.utils.recursiveOmitUndefined),
        }))
        .map(ADCSDK.utils.recursiveOmitUndefined) ?? [],
    global_rules: Object.fromEntries(
      input.global_rules
        ?.map((globalRule) => [globalRule.id, globalRule.plugins![0]])
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
};
