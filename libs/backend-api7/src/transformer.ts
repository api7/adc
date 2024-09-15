import * as ADCSDK from '@api7/adc-sdk';
import { attempt, isError } from 'lodash';

import * as typing from './typing';

export class ToADC {
  private static transformLabels(
    labels?: ADCSDK.Labels,
  ): Record<string, string> {
    if (!labels) return undefined;
    return Object.entries(labels).reduce((pv, [key, value]) => {
      const res = attempt(JSON.parse, value as string);
      pv[key] = !isError(res) && Array.isArray(res) ? res : value;
      return pv;
    }, {});
  }

  public transformRoute(route: typing.Route): ADCSDK.Route {
    return ADCSDK.utils.recursiveOmitUndefined<ADCSDK.Route>({
      uris: route?.paths?.[0] ? [route?.paths?.[0]] : undefined,
      name: route.name,
      description: route.desc,
      labels: ToADC.transformLabels(route.labels),
      methods: route.methods,
      enable_websocket: route.enable_websocket,
      plugins: route.plugins,
      priority: route.priority,
      metadata: { id: route.route_id },
    });
  }

  public transformStreamRoute(route: typing.StreamRoute): ADCSDK.StreamRoute {
    return ADCSDK.utils.recursiveOmitUndefined<ADCSDK.StreamRoute>({
      name: route.name,
      description: route.desc,
      labels: ToADC.transformLabels(route.labels),
      server_addr: route.server_addr,
      server_port: route.server_port,
      remote_addr: route.remote_addr,
      metadata: { id: route.stream_route_id },
    });
  }

  public transformService(service: typing.Service): ADCSDK.Service {
    return ADCSDK.utils.recursiveOmitUndefined<ADCSDK.Service>({
      name: service.name,
      description: service.desc,
      labels: ToADC.transformLabels(service.labels),
      upstream: service.upstream,
      plugins: service.plugins,
      path_prefix: service.path_prefix,
      strip_path_prefix: service.strip_path_prefix,
      hosts: service.hosts,
      routes: service.routes?.map((item) => this.transformRoute(item)),
      stream_routes: service.stream_routes?.map((item) =>
        this.transformStreamRoute(item),
      ),
      metadata: { id: service.service_id },
    });
  }

  public transformConsumer(consumer: typing.Consumer): ADCSDK.Consumer {
    return ADCSDK.utils.recursiveOmitUndefined<ADCSDK.Consumer>({
      username: consumer.username,
      description: consumer.desc,
      labels: ToADC.transformLabels(consumer.labels),
      plugins: consumer.plugins,
      credentials: consumer.credentials?.map((item) =>
        this.transformConsumerCredential(item),
      ),
    });
  }

  public transformConsumerCredential(
    credential: typing.ConsumerCredential,
  ): ADCSDK.ConsumerCredential {
    const [pluginName, config] = Object.entries(credential.plugins)[0];
    return ADCSDK.utils.recursiveOmitUndefined<ADCSDK.ConsumerCredential>({
      name: credential.name,
      description: credential.desc,
      labels: credential.labels,
      type: pluginName as ADCSDK.ConsumerCredential['type'],
      config,
      metadata: { id: credential.id },
    });
  }

  public transformSSL(ssl: typing.SSL): ADCSDK.SSL {
    return ADCSDK.utils.recursiveOmitUndefined<ADCSDK.SSL>({
      labels: ToADC.transformLabels(ssl.labels),
      type: ssl.type,
      snis: ssl.snis,
      certificates: ssl?.cert
        ? [{ certificate: ssl.cert, key: '' }]
        : undefined,
      client: ssl.client
        ? {
            ca: ssl.client.ca,
            depth: ssl.client.depth,
            skip_mtls_uri_regex: undefined,
          }
        : undefined,
      metadata: { id: ssl.id },
    });
  }

  public transformGlobalRule(
    globalRules: Array<typing.GlobalRule>,
  ): Record<string, ADCSDK.GlobalRule> {
    return Object.fromEntries(
      globalRules.map((globalRule) => {
        const pluginName = Object.keys(globalRule.plugins)[0];
        const pluginConfig = Object.values(globalRule.plugins)[0];
        return [pluginName, pluginConfig];
      }),
    );
  }

  public transformPluginMetadatas(
    pluginMetadatas: typing.PluginMetadata,
  ): Record<string, ADCSDK.PluginMetadata> {
    return pluginMetadatas;
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

  public transformRoute(route: ADCSDK.Route, serviceId: string): typing.Route {
    return ADCSDK.utils.recursiveOmitUndefined<typing.Route>({
      route_id: ADCSDK.utils.generateId(route.name),
      name: route.name,
      desc: route.description,
      labels: FromADC.transformLabels(route.labels),
      methods: route.methods,
      enable_websocket: route.enable_websocket,
      plugins: route.plugins,
      service_id: serviceId,
      paths: [route.uris[0]],
      priority: route.priority,
    });
  }

  public transformStreamRoute(
    route: ADCSDK.StreamRoute,
    serviceId: string,
  ): typing.StreamRoute {
    return ADCSDK.utils.recursiveOmitUndefined<typing.StreamRoute>({
      stream_route_id: ADCSDK.utils.generateId(route.name),
      name: route.name,
      desc: route.description,
      labels: FromADC.transformLabels(route.labels),
      service_id: serviceId,
      plugins: route.plugins,
      server_addr: route.server_addr,
      server_port: route.server_port,
      remote_addr: route.remote_addr,
    });
  }

  public transformService(service: ADCSDK.Service): typing.Service {
    return ADCSDK.utils.recursiveOmitUndefined({
      service_id: ADCSDK.utils.generateId(service.name),
      name: service.name,
      desc: service.description,
      labels: FromADC.transformLabels(service.labels),
      upstream: service.upstream,
      plugins: service.plugins,
      path_prefix: service.path_prefix,
      strip_path_prefix: service.strip_path_prefix,
      hosts: service.hosts,
      type: ['tcp', 'udp', 'tls'].includes(service?.upstream?.scheme)
        ? 'stream'
        : 'http',
    });
  }

  public transformConsumer(consumer: ADCSDK.Consumer): typing.Consumer {
    return ADCSDK.utils.recursiveOmitUndefined({
      username: consumer.username,
      desc: consumer.description,
      labels: FromADC.transformLabels(consumer.labels),
      plugins: consumer.plugins,
    });
  }

  public transformConsumerCredential(
    credential: ADCSDK.ConsumerCredential,
  ): typing.ConsumerCredential {
    return ADCSDK.utils.recursiveOmitUndefined<typing.ConsumerCredential>({
      id: ADCSDK.utils.generateId(credential.name),
      name: credential.name,
      desc: credential.description,
      labels: FromADC.transformLabels(credential.labels),
      plugins: {
        [credential.type]: credential.config,
      },
    });
  }

  public transformSSL(ssl: ADCSDK.SSL): typing.SSL {
    return ADCSDK.utils.recursiveOmitUndefined({
      id: ADCSDK.utils.generateId(ssl.snis.join(',')),
      labels: FromADC.transformLabels(ssl.labels),
      status: 1,
      certificates: undefined,
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

  public transformGlobalRule(
    globalRules: Record<string, ADCSDK.GlobalRule>,
  ): Array<typing.GlobalRule> {
    return Object.entries(globalRules).map(([pluginName, pluginConfig]) => ({
      plugins: { [pluginName]: pluginConfig },
    })) as Array<typing.GlobalRule>;
  }

  public transformPluginMetadatas(
    pluginMetadatas: Record<string, ADCSDK.PluginMetadata>,
  ): typing.PluginMetadata {
    return ADCSDK.utils.recursiveOmitUndefined(pluginMetadatas);
  }
}
