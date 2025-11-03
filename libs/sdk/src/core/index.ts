import { ResourceType } from './resource';
import {
  Consumer,
  ConsumerCredential,
  GlobalRule,
  Labels,
  PluginMetadata,
  Plugins,
  Route,
  SSL,
  Service,
  StreamRoute,
  Upstream,
} from './schema';

export * from './differ';
export * from './resource';

export type {
  Labels,
  Plugin,
  Plugins,
  Expr,
  Route,
  Service,
  UpstreamBalancer,
  UpstreamScheme,
  UpstreamPassHost,
  UpstreamNode,
  UpstreamTimeout,
  Upstream,
  UpstreamHealthCheck,
  SSLCertificate,
  SSL,
  GlobalRule,
  PluginMetadata,
  ConsumerCredential,
  Consumer,
  StreamRoute,
  Configuration,
  InternalConfiguration,
} from './schema';

export interface PluginConfig {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  plugins: Plugins;
}

export interface ConsumerGroup {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  plugins: Plugins;

  consumers?: Array<Consumer>;
}

export type ResourceFor<T extends ResourceType> = T extends ResourceType.SERVICE
  ? Service
  : T extends ResourceType.SSL
    ? SSL
    : T extends ResourceType.CONSUMER
      ? Consumer
      : T extends ResourceType.GLOBAL_RULE
        ? GlobalRule
        : T extends ResourceType.PLUGIN_METADATA
          ? PluginMetadata
          : T extends ResourceType.ROUTE
            ? Route
            : T extends ResourceType.STREAM_ROUTE
              ? StreamRoute
              : T extends ResourceType.UPSTREAM
                ? Upstream
                : T extends ResourceType.CONSUMER_GROUP
                  ? ConsumerGroup
                  : T extends ResourceType.PLUGIN_CONFIG
                    ? PluginConfig
                    : T extends ResourceType.CONSUMER_CREDENTIAL
                      ? ConsumerCredential
                      : never;
