import * as ADCSDK from '@api7/adc-sdk';
import { isEmpty, unset } from 'lodash';

import * as typing from './typing';

export class ToADC {
  public transformRoute(route: typing.Route): ADCSDK.Route {
    return ADCSDK.utils.recursiveOmitUndefined({
      name: route.name ?? route.id,
      description: route.desc,
      labels: route.labels,

      uris: route.uri ? [route.uri] : route.uris,
      hosts: route.host ? [route.host] : route.hosts,
      priority: route.priority,
      timeout: route.timeout,
      vars: route.vars,
      methods: route.methods,
      enable_websocket: route.enable_websocket,
      remote_addrs: route.remote_addr
        ? [route.remote_addr]
        : route.remote_addrs,
      plugins: route.plugins,
      plugin_config_id: route.plugin_config_id,
      filter_func: route.filter_func,

      metadata: { id: route.id },
    } as ADCSDK.Route);
  }

  public transformService(service: typing.Service): ADCSDK.Service {
    return ADCSDK.utils.recursiveOmitUndefined({
      name: service.name ?? service.id,
      description: service.desc,
      labels: service.labels,

      upstream: service.upstream,
      plugins: service.plugins,

      metadata: { id: service.id },
    } as ADCSDK.Service);
  }

  public transformConsumer(
    consumer: typing.Consumer,
    removeGroupId = false,
  ): ADCSDK.Consumer {
    return ADCSDK.utils.recursiveOmitUndefined({
      username: consumer.username,
      description: consumer.desc,
      labels: consumer.labels,

      plugins: consumer.plugins,
      group_id: !removeGroupId ? consumer.group_id : undefined,
    });
  }

  public transformSSL(ssl: typing.SSL): ADCSDK.SSL {
    const certificates: Array<ADCSDK.SSLCertificate> = [
      {
        certificate: ssl.cert,
        key: ssl.key,
      },
      ...(ssl.certs?.map<ADCSDK.SSLCertificate>((cert, idx) => ({
        certificate: cert,
        key: ssl.keys[idx],
      })) ?? []),
    ];

    return ADCSDK.utils.recursiveOmitUndefined({
      labels: ssl.labels,

      type: ssl.type,
      snis: ssl.sni ? [ssl.sni] : ssl.snis,
      certificates: certificates,
      client: ssl.client
        ? {
            ca: ssl.client.ca,
            depth: ssl.client.depth,
            skip_mtls_uri_regex: ssl.client.skip_mtls_uri_regex,
          }
        : undefined,
      ssl_protocols: ssl.ssl_protocols,
    });
  }

  public transformConsumerGroup(
    consumerGroup: typing.ConsumerGroup,
    consumers?: Array<typing.Consumer>,
  ): ADCSDK.ConsumerGroup {
    const adcConsumerGroup: ADCSDK.ConsumerGroup =
      ADCSDK.utils.recursiveOmitUndefined({
        name: (consumerGroup.labels?.ADC_NAME as string) ?? consumerGroup.id,
        description: consumerGroup.desc,
        labels: consumerGroup.labels,
        plugins: consumerGroup.plugins,

        consumers: consumers
          .filter(
            (consumer) =>
              consumer.group_id && consumer.group_id === consumerGroup.id,
          )
          .map((consumer) => this.transformConsumer(consumer, true)),
      });

    unset(adcConsumerGroup, 'labels.ADC_NAME');
    if (isEmpty(adcConsumerGroup.labels)) unset(adcConsumerGroup, 'labels');

    return adcConsumerGroup;
  }

  public transformGlobalRule(globalRule: typing.GlobalRule): ADCSDK.GlobalRule {
    return ADCSDK.utils.recursiveOmitUndefined({
      name: globalRule.id,
      plugins: globalRule.plugins,
    });
  }

  public transformStreamRoute(
    streamRoute: typing.StreamRoute,
  ): ADCSDK.StreamRoute {
    return ADCSDK.utils.recursiveOmitUndefined({
      name: streamRoute.name ?? streamRoute.id,
      description: streamRoute.desc,
      labels: streamRoute.labels,

      remote_addr: streamRoute.remote_addr,
      server_addr: streamRoute.server_addr,
      server_port: streamRoute.server_port,
      sni: streamRoute.sni,

      metadata: { id: streamRoute.id },
    } as ADCSDK.StreamRoute);
  }

  public transformUpstream(upstream: typing.Upstream): ADCSDK.Upstream {
    const defaultPortMap: Record<string, number> = {
      http: 80,
      https: 443,
      grpc: 80,
      grpcs: 443,
    };
    const nodes = Array.isArray(upstream.nodes)
      ? upstream.nodes
      : Object.keys(upstream.nodes).map<ADCSDK.UpstreamNode>((node) => {
          const hostport = node.split(':');
          return {
            host: hostport[0],
            port:
              hostport.length === 2
                ? parseInt(hostport[1])
                : defaultPortMap[upstream.scheme]
                  ? defaultPortMap[upstream.scheme]
                  : 80,
            weight: upstream.nodes[node],
          };
        });
    return ADCSDK.utils.recursiveOmitUndefined({
      name: upstream.name ?? upstream.id,
      description: upstream.desc,
      labels: upstream.labels,

      type: upstream.type,
      hash_on: upstream.hash_on,
      key: upstream.key,
      checks: upstream.checks,
      nodes,
      scheme: upstream.scheme,
      retries: upstream.retries,
      retry_timeout: upstream.retry_timeout,
      timeout: upstream.timeout,
      tls: upstream.tls
        ? {
            client_cert_id: upstream.tls.client_cert_id,
            cert: upstream.tls.client_cert,
            key: upstream.tls.client_key,
            verify: upstream.tls.verify,
          }
        : undefined,
      keepalive_pool: upstream.keepalive_pool
        ? {
            size: upstream.keepalive_pool.size,
            idle_timeout: upstream.keepalive_pool.idle_timeout,
            requests: upstream.keepalive_pool.requests,
          }
        : undefined,
      pass_host: upstream.pass_host,
      upstream_host: upstream.upstream_host,

      service_name: upstream.service_name,
      discovery_type: upstream.discovery_type,
      discovery_args: upstream.discovery_args,
    });
  }
}

export class FromADC {
  private static transformLabels(
    labels?: ADCSDK.Labels,
  ): Record<string, string> {
    if (!labels) return undefined;
    return Object.entries(labels).reduce((pv, [key, value]) => {
      pv[key] = typeof value === 'string' ? value : JSON.stringify(value);
      return pv;
    }, {});
  }

  public transformRoute(route: ADCSDK.Route): typing.Route {
    return ADCSDK.utils.recursiveOmitUndefined({
      ...route,
      id: undefined,
      labels: FromADC.transformLabels(route.labels),
      status: 1,

      desc: route.description,
      description: undefined,
    });
  }

  public transformService(
    service: ADCSDK.Service,
  ): [typing.Service, Array<typing.Route>, Array<typing.StreamRoute>] {
    const serviceId = ADCSDK.utils.generateId(service.name);
    const routes: Array<typing.Route> =
      service.routes
        ?.map(this.transformRoute)
        .map((route) => ({ ...route, service_id: serviceId })) ?? [];
    const streamRoutes: Array<typing.StreamRoute> =
      service.stream_routes
        ?.map(this.transformStreamRoute)
        .map((route) => ({ ...route, service_id: serviceId })) ?? [];
    return [
      ADCSDK.utils.recursiveOmitUndefined({
        ...service,
        id: undefined,
        labels: FromADC.transformLabels(service.labels),
        routes: undefined,
        stream_routes: undefined,

        desc: service.description,
        description: undefined,
      }),
      routes,
      streamRoutes,
    ];
  }

  public transformConsumer(consumer: ADCSDK.Consumer): typing.Consumer {
    return ADCSDK.utils.recursiveOmitUndefined({
      ...consumer,
      id: undefined,
      labels: FromADC.transformLabels(consumer.labels),

      desc: consumer.description,
      description: undefined,
    });
  }

  public transformSSL(ssl: ADCSDK.SSL): typing.SSL {
    return ADCSDK.utils.recursiveOmitUndefined({
      ...ssl,
      id: undefined,
      labels: FromADC.transformLabels(ssl.labels),
      status: 1,
      certificates: undefined,

      cert: ssl.certificates[0].certificate,
      key: ssl.certificates[0].key,
      ...(ssl.certificates.length > 1
        ? {
            certs: ssl.certificates
              .slice(1)
              .map((certificate) => certificate.certificate),
            keys: ssl.certificates
              .slice(1)
              .map((certificate) => certificate.key),
          }
        : {}),
    });
  }

  public transformConsumerGroup(
    consumerGroup: ADCSDK.ConsumerGroup,
  ): [typing.ConsumerGroup, Array<typing.Consumer>] {
    const consumerGroupId = ADCSDK.utils.generateId(consumerGroup.name);
    const consumers: Array<typing.Consumer> = consumerGroup.consumers
      ?.map(this.transformConsumer)
      .map((consumer) => ({ ...consumer, group_id: consumerGroupId }));

    return [
      ADCSDK.utils.recursiveOmitUndefined({
        ...consumerGroup,
        id: undefined,
        labels: {
          ...FromADC.transformLabels(consumerGroup.labels),
          ADC_NAME: consumerGroup.name,
        },
        name: undefined,
        consumers: undefined,
      }),
      consumers,
    ];
  }

  public transformGlobalRules(
    globalRules: Record<string, ADCSDK.GlobalRule>,
  ): Array<typing.GlobalRule> {
    return Object.entries(globalRules).reduce<Array<typing.GlobalRule>>(
      (pv, [key, value]) => {
        pv.push(
          ADCSDK.utils.recursiveOmitUndefined({
            id: undefined,
            plugins: {
              [key]: value as ADCSDK.Plugin,
            },
          }),
        );
        return pv;
      },
      [],
    );
  }

  public transformPluginMetadatas(
    pluginMetadatas: Record<string, ADCSDK.PluginMetadata>,
  ): Array<typing.PluginMetadata> {
    return Object.entries(pluginMetadatas).reduce<Array<typing.PluginMetadata>>(
      (pv, [key, value]) => {
        pv.push(
          ADCSDK.utils.recursiveOmitUndefined({
            id: undefined,
            ...value,
            __plugin_name: key,
          }),
        );
        return pv;
      },
      [],
    );
  }

  public transformStreamRoute(
    streamRoute: ADCSDK.StreamRoute,
  ): typing.StreamRoute {
    return ADCSDK.utils.recursiveOmitUndefined({
      ...streamRoute,
      id: undefined,
      labels: FromADC.transformLabels(streamRoute.labels),
    });
  }

  public transformUpstream(upstream: ADCSDK.Upstream): typing.Upstream {
    return ADCSDK.utils.recursiveOmitUndefined({
      ...upstream,
      id: undefined,
      labels: FromADC.transformLabels(upstream.labels),
    });
  }
}
