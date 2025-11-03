import {
  GlobalRule as ADCGlobalRule,
  PluginMetadata as ADCPluginMetadata,
  Expr,
  Labels,
  Plugins,
  ResourceType,
  UpstreamBalancer,
  UpstreamHealthCheck,
  UpstreamNode,
  UpstreamPassHost,
  UpstreamScheme,
  UpstreamTimeout,
} from '@api7/adc-sdk';

export const ADC_UPSTREAM_SERVICE_ID_LABEL = '__ADC_UPSTREAM_SERVICE_ID';

export interface Route {
  id: string;
  name?: string;
  desc?: string;
  labels?: Record<string, string>;

  // matcher
  uri?: string;
  uris?: Array<string>;
  host?: string;
  hosts?: Array<string>;
  methods?: Array<string>;
  remote_addr?: string;
  remote_addrs?: Array<string>;
  vars?: Expr;
  filter_func?: string;

  // upstream and policies
  script?: string;
  script_id?: string;
  plugins?: Plugins;
  plugin_config_id?: string;
  upstream?: InlineUpstream;
  upstream_id?: string;
  service_id?: string;
  timeout?: UpstreamTimeout;

  // misc
  enable_websocket?: boolean;
  priority?: number;
  status?: number;
}
export interface Service {
  id: string;
  name?: string;
  desc?: string;
  labels?: Labels;

  hosts?: Array<string>;
  upstream?: InlineUpstream;
  upstream_id?: string;
  plugins?: Plugins;
  script?: string;
  enable_websocket?: boolean;

  // internal use only
  upstreams?: Array<InlineUpstream>;
}
export interface ConsumerCredential {
  id?: string;
  name: string;
  desc?: string;
  labels?: Labels;

  plugins?: Plugins;
}
export interface Consumer {
  username: string;
  desc?: string;
  labels?: Labels;

  group_id?: string;
  plugins?: Plugins;
  credentials?: Array<ConsumerCredential>;
}
export interface SSL {
  id: string;
  labels?: Labels;

  type?: 'server' | 'client';
  sni?: string;
  snis?: Array<string>;
  cert?: string;
  certs?: Array<string>;
  key?: string;
  keys?: Array<string>;
  client?: {
    ca: string;
    depth: number;
    skip_mtls_uri_regex?: Array<string>;
  };
  ssl_protocols?: Array<string>;

  status: number;
}
export interface PluginConfig {
  id: string;
  name?: string;
  desc?: string;
  labels?: Labels;

  plugins: Plugins;
}
export interface ConsumerGroup {
  id: string;
  desc?: string;
  labels?: Labels;

  plugins: Plugins;
}
export type GlobalRule = ADCGlobalRule;
export type PluginMetadata = ADCPluginMetadata;
export interface StreamRoute {
  id: string;
  desc?: string;
  labels?: Labels;
  //name: string; // As of 3.9.1, APISIX does not support name on the stream route

  remote_addr?: string;
  server_addr?: string;
  server_port?: number;
  sni?: string;
  upstream?: InlineUpstream;
  upstream_id?: string;
  service_id?: string;

  plugins?: Plugins;
  protocol?: {
    name: string;
    superior_id?: string;
    conf?: object;
    logger?: Array<{
      name?: string;
      filter?: Array<unknown>;
      conf: object;
    }>;
  };
}
export interface Upstream {
  id: string;
  name?: string;
  desc?: string;
  labels?: Labels;

  nodes?: Array<UpstreamNode> | Record<string, number>;
  scheme?: UpstreamScheme;
  type?: UpstreamBalancer;
  hash_on?: string;
  key?: string;
  checks?: UpstreamHealthCheck;

  discovery_type?: string;
  service_name?: string;
  discovery_args?: Record<string, unknown>;

  pass_host?: UpstreamPassHost;
  upstream_host?: string;
  retries?: number;
  retry_timeout?: number;
  timeout?: UpstreamTimeout;
  tls?: {
    client_cert_id?: string;
    client_cert?: string;
    client_key?: string;
    verify?: boolean;
  };
  keepalive_pool?: {
    size: number;
    idle_timeout: number;
    requests: number;
  };
}
export type InlineUpstream = Omit<Upstream, 'id'>;

export interface ListResponse<T> {
  list: Array<{
    key: string;
    value: T;
    createdIndex: number;
    modifiedIndex: number;
  }>;
  total: number;
}

export interface Resources {
  [ResourceType.ROUTE]?: Array<Route>;
  [ResourceType.SERVICE]?: Array<Service>;
  [ResourceType.CONSUMER]?: Array<Consumer>;
  [ResourceType.SSL]?: Array<SSL>;
  [ResourceType.GLOBAL_RULE]?: GlobalRule;
  [ResourceType.PLUGIN_CONFIG]?: Array<PluginConfig>;
  [ResourceType.CONSUMER_GROUP]?: Array<ConsumerGroup>;
  [ResourceType.PLUGIN_METADATA]?: PluginMetadata;
  [ResourceType.STREAM_ROUTE]?: Array<StreamRoute>;
  [ResourceType.UPSTREAM]?: Array<Upstream>;
}
