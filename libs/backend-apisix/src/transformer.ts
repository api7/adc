import * as ADCSDK from '@api7/adc-sdk';
import { filter, isEmpty, unset } from 'lodash';

import * as typing from './typing';

export class ToADC {
  private static transformLabels(
    labels?: ADCSDK.Labels,
  ): ADCSDK.Labels | undefined {
    if (!labels) return undefined;
    const filteredLabels = filter(
      labels,
      (val, key) => key !== '__ADC_NAME',
    ) as unknown as ADCSDK.Labels;
    return Object.values(filteredLabels).length > 0
      ? filteredLabels
      : undefined;
  }

  public transformRoute(route: typing.Route): ADCSDK.Route {
    return ADCSDK.utils.recursiveOmitUndefined({
      id: route.id,
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
    } as ADCSDK.Route);
  }

  public transformService(service: typing.Service): ADCSDK.Service {
    return ADCSDK.utils.recursiveOmitUndefined({
      id: service.id,
      name: service.name ?? service.id,
      description: service.desc,
      labels: service.labels,

      hosts: service.hosts,

      upstream: service.upstream
        ? this.transformUpstream(service.upstream)
        : undefined,
      upstreams: service.upstreams,
      plugins: service.plugins,
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

      credentials: consumer.credentials
        ?.map((item) => this.transformConsumerCredential(item))
        .filter((item) => !!item),
    } as ADCSDK.Consumer);
  }

  public transformConsumerCredential(
    credential: typing.ConsumerCredential,
  ): ADCSDK.ConsumerCredential | undefined {
    if (!credential.plugins || Object.keys(credential.plugins).length <= 0)
      return;

    const [pluginName, config] = Object.entries(credential.plugins)[0];
    if (
      !['key-auth', 'basic-auth', 'jwt-auth', 'hmac-auth'].includes(pluginName)
    )
      return;
    return ADCSDK.utils.recursiveOmitUndefined<ADCSDK.ConsumerCredential>({
      id: credential.id,
      name: credential.name,
      description: credential.desc,
      labels: credential.labels,
      type: pluginName as ADCSDK.ConsumerCredential['type'],
      config: config as ADCSDK.ConsumerCredential['config'],
    });
  }

  public transformSSL(ssl: typing.SSL): ADCSDK.SSL {
    const certificates: Array<ADCSDK.SSLCertificate> = [
      {
        certificate: ssl.cert!,
        key: ssl.key!,
      },
      ...(ssl.certs?.map<ADCSDK.SSLCertificate>((cert, idx) => ({
        certificate: cert,
        key: ssl.keys![idx]!,
      })) ?? []),
    ];

    return ADCSDK.utils.recursiveOmitUndefined({
      id: ssl.id,
      labels: ssl.labels,

      type: ssl.type,
      snis: ssl.sni ? [ssl.sni] : ssl.snis ?? [],
      certificates: certificates,
      client: ssl.client
        ? {
            ca: ssl.client.ca,
            depth: ssl.client.depth,
            skip_mtls_uri_regex: ssl.client.skip_mtls_uri_regex,
          }
        : undefined,
      ssl_protocols: ssl.ssl_protocols as ('TLSv1.1' | 'TLSv1.2' | 'TLSv1.3')[],
    });
  }

  public transformConsumerGroup(
    consumerGroup: typing.ConsumerGroup,
    consumers?: Array<typing.Consumer>,
  ): ADCSDK.ConsumerGroup {
    const adcConsumerGroup =
      ADCSDK.utils.recursiveOmitUndefined<ADCSDK.ConsumerGroup>({
        id: consumerGroup.id,
        name: (consumerGroup.labels?.ADC_NAME as string) ?? consumerGroup.id,
        description: consumerGroup.desc,
        labels: consumerGroup.labels,
        plugins: consumerGroup.plugins,

        consumers: consumers
          ? consumers
              .filter(
                (consumer) =>
                  consumer.group_id && consumer.group_id === consumerGroup.id,
              )
              .map((consumer) => this.transformConsumer(consumer, true))
          : undefined,
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
      id: streamRoute.id,
      name: streamRoute.labels?.__ADC_NAME ?? streamRoute.id,
      description: streamRoute.desc,
      labels: ToADC.transformLabels(streamRoute.labels),

      remote_addr: streamRoute.remote_addr,
      server_addr: streamRoute.server_addr,
      server_port: streamRoute.server_port,
      sni: streamRoute.sni,

      metadata: { id: streamRoute.id },
    } as ADCSDK.StreamRoute);
  }

  public transformUpstream(
    upstream: typing.Upstream | typing.InlineUpstream,
  ): ADCSDK.Upstream {
    const defaultPortMap: Record<string, number> = {
      http: 80,
      https: 443,
      grpc: 80,
      grpcs: 443,
    };
    const nodes = upstream.nodes
      ? Array.isArray(upstream.nodes)
        ? upstream.nodes
        : Object.entries(
            upstream.nodes as Record<string, number>,
          ).map<ADCSDK.UpstreamNode>(([node, weight]) => {
            const hostport = node.split(':');
            return {
              host: hostport[0],
              port:
                hostport.length === 2
                  ? parseInt(hostport[1])
                  : defaultPortMap[upstream.scheme!]
                    ? defaultPortMap[upstream.scheme!]
                    : 80,
              weight: weight,
            };
          })
      : undefined;
    const labels: Record<string, string | string[]> = {
      ...(upstream.labels ?? {}),
    };
    delete (labels as Record<string, unknown>)[
      typing.ADC_UPSTREAM_SERVICE_ID_LABEL
    ];
    return ADCSDK.utils.recursiveOmitUndefined({
      ...{ id: 'id' in upstream ? upstream.id : undefined },
      name: upstream.name,
      description: upstream.desc,
      labels: Object.keys(labels).length > 0 ? labels : undefined,

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
  ): Record<string, string> | undefined {
    if (!labels) return undefined;
    return Object.entries(labels).reduce(
      (pv, [key, value]) => {
        pv[key] = typeof value === 'string' ? value : JSON.stringify(value);
        return pv;
      },
      {} as Record<string, string>,
    );
  }

  public transformRoute(route: ADCSDK.Route, parentId: string): typing.Route {
    return ADCSDK.utils.recursiveOmitUndefined<typing.Route>({
      id: route.id!,
      name: route.name,
      desc: route.description,
      labels: FromADC.transformLabels(route.labels),
      uris: route.uris,
      hosts: route.hosts,
      methods: route.methods,
      remote_addrs: route.remote_addrs,
      vars: route.vars,
      filter_func: route.filter_func,

      service_id: parentId,
      enable_websocket: route.enable_websocket,
      plugins: route.plugins,
      priority: route.priority,
      timeout: route.timeout,
      status: 1,
    });
  }

  public transformService(
    service: ADCSDK.Service,
  ): [typing.Service, typing.Upstream | undefined] {
    return [
      ADCSDK.utils.recursiveOmitUndefined<typing.Service>({
        id: service.id!,
        name: service.name,
        desc: service.description,
        labels: FromADC.transformLabels(service.labels),
        upstream_id: service.id,
        plugins: service.plugins,
        hosts: service.hosts,
      }),
      service.upstream
        ? ({
            ...this.transformUpstream(service.upstream),
            id: service.id,
            name: service.name,
          } as typing.Upstream)
        : undefined,
    ];
  }

  public transformConsumer(consumer: ADCSDK.Consumer): typing.Consumer {
    return ADCSDK.utils.recursiveOmitUndefined({
      username: consumer.username,
      desc: consumer.description,
      labels: FromADC.transformLabels(consumer.labels),
      plugins: consumer.plugins,
    } as typing.Consumer);
  }

  public transformConsumerCredential(
    credential: ADCSDK.ConsumerCredential,
  ): typing.ConsumerCredential {
    return ADCSDK.utils.recursiveOmitUndefined<typing.ConsumerCredential>({
      id: credential.id,
      name: credential.name,
      desc: credential.description,
      labels: FromADC.transformLabels(credential.labels),
      plugins: {
        [credential.type]: credential.config,
      },
    });
  }

  public transformSSL(ssl: ADCSDK.SSL): typing.SSL {
    return ADCSDK.utils.recursiveOmitUndefined<typing.SSL>({
      id: ssl.id!,
      labels: FromADC.transformLabels(ssl.labels),
      status: 1,
      type: ssl.type,

      snis: ssl.snis,
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
      client: ssl.client,
    });
  }

  public transformConsumerGroup(
    consumerGroup: ADCSDK.ConsumerGroup,
  ): [typing.ConsumerGroup, Array<typing.Consumer>] {
    const consumerGroupId = ADCSDK.utils.generateId(consumerGroup.name);
    const consumers: Array<typing.Consumer> = (consumerGroup.consumers ?? [])
      .map((c) => this.transformConsumer(c))
      .map((consumer) => ({ ...consumer, group_id: consumerGroupId }));

    return [
      ADCSDK.utils.recursiveOmitUndefined<typing.ConsumerGroup>({
        ...consumerGroup,
        id: consumerGroupId,
        labels: {
          ...FromADC.transformLabels(consumerGroup.labels),
          ADC_NAME: consumerGroup.name,
        },
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
          ADCSDK.utils.recursiveOmitUndefined<typing.GlobalRule>({
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
          ADCSDK.utils.recursiveOmitUndefined<typing.PluginMetadata>({
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
    parentId: string,
    injectName = true,
  ): typing.StreamRoute {
    const labels = FromADC.transformLabels(streamRoute.labels);
    return ADCSDK.utils.recursiveOmitUndefined({
      id: undefined,
      desc: streamRoute.description,
      labels: injectName
        ? {
            ...labels,
            __ADC_NAME: streamRoute.name,
          }
        : labels,
      plugins: streamRoute.plugins,
      remote_addr: streamRoute.remote_addr,
      server_addr: streamRoute.server_addr,
      server_port: streamRoute.server_port,
      sni: streamRoute.sni,
      service_id: parentId,
    } as unknown as typing.StreamRoute);
  }

  public transformUpstream(
    upstream: ADCSDK.Upstream,
  ): Omit<typing.Upstream, 'id'> {
    const { id: _omit, ...rest } = upstream as { id?: string } & Omit<
      ADCSDK.Upstream,
      'id'
    >;
    return ADCSDK.utils.recursiveOmitUndefined<Omit<typing.Upstream, 'id'>>({
      ...rest,
      labels: FromADC.transformLabels(upstream.labels),
    });
  }
}
