import {
  PluginMetadata as ADCPluginMetadata,
  Expr,
  Labels,
  Plugins,
  UpstreamBalancer,
  UpstreamHealthCheck,
  UpstreamNode,
  UpstreamPassHost,
  UpstreamScheme,
  UpstreamTimeout,
} from '@api7/adc-sdk';

export interface Route {
  id?: string;
  name: string;
  desc?: string;
  labels?: Record<string, string>;
  service_id: string;
  route_id: string;

  // policies
  plugins?: Plugins;

  // matcher
  paths: Array<string>;
  methods?: Array<string>;
  vars: Expr;

  // misc
  enable_websocket?: boolean;
  priority?: number;
  timeout?: UpstreamTimeout;
}
export interface StreamRoute {
  id?: string;
  name: string;
  desc: string;
  labels?: Record<string, string>;
  service_id: string;
  stream_route_id: string;

  plugins?: Plugins;

  server_addr: string;
  server_port: number;
  remote_addr: string;
}
export interface Service {
  id?: string;
  name: string;
  desc?: string;
  labels?: Labels;
  type?: 'http' | 'stream';
  hosts?: Array<string>;
  path_prefix?: string;
  strip_path_prefix?: boolean;
  upstream: Upstream;
  plugins?: Plugins;
  version?: string;
  service_version_id?: string;
  service_id: string;
  routes?: Array<Route>;
  stream_routes?: Array<StreamRoute>;

  // multiple upstreams for canary release
  upstreams?: Array<Upstream>;
}
export interface ConsumerCredential {
  id?: string;
  name: string;
  desc?: string;
  labels?: Labels;

  plugins: Plugins;
}
export interface Consumer {
  username: string;
  desc?: string;
  labels?: Labels;
  plugins?: Plugins;

  credentials?: Array<ConsumerCredential>;
}
export interface SSL {
  id?: string;
  labels?: Labels;
  type: 'server' | 'client';
  cert?: string;
  certs?: Array<string>;
  key?: string;
  keys?: Array<string>;
  client?: {
    ca: string;
    depth: number;
  };
  snis?: string[];
}
export interface GlobalRule {
  plugins: Plugins;
}
export type PluginMetadata = Record<string, ADCPluginMetadata>;
export interface Upstream {
  id?: string;
  name: string;
  desc?: string;
  labels?: Labels;

  nodes?: Array<Omit<UpstreamNode, 'metadata'>>;
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
    client_cert?: string;
    client_key?: string;
    client_cert_id?: string;
    verify?: boolean;
  };
  keepalive_pool?: {
    size: number;
    idle_timeout: number;
    requests: number;
  };
}
export interface Resources {
  services: Array<Service>;
  consumers: Array<Consumer>;
  ssls: Array<SSL>;
  globalRules: Array<GlobalRule>;
  pluginMetadatas: PluginMetadata;
}

export interface ListResponse<T> {
  list: Array<T>;
  total: number;
}
