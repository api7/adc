export * from './differ';
export * from './resource';

export type Labels = Record<string, string | Array<string>>;
export type Plugin = Record<string, unknown>;
export type Plugins = Record<string, Plugin>;
export type Expr = Array<unknown>;

export interface Route {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  hosts?: Array<string>;
  uris: Array<string>;
  priority?: number;
  timeout?: UpstreamTimeout;
  vars?: Expr;
  methods?: Array<string>;
  enable_websocket?: boolean;
  remote_addrs?: Array<string>;
  plugins?: Plugins;
  filter_func?: string;
  service_id?: string;

  metadata?: ResourceMetadata;
}

export interface Service {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  upstream?: Upstream;
  upstreams?: Array<Upstream>;
  plugins?: Plugins;
  path_prefix?: string;
  strip_path_prefix?: boolean;
  hosts?: Array<string>;

  routes?: Array<Route>;
  stream_routes?: Array<StreamRoute>;

  metadata?: ResourceMetadata;
}

export type UpstreamBalancer = 'roundrobin' | 'chash' | 'least_conn' | 'ewma';
export type UpstreamScheme =
  | 'grpc'
  | 'grpcs'
  | 'http'
  | 'https'
  | 'tcp'
  | 'tls'
  | 'udp'
  | 'kafka';
export type UpstreamPassHost = 'pass' | 'node' | 'rewrite';
export interface UpstreamNode {
  host: string;
  port: number;
  weight: number;
  priority?: number;
  metadata?: { [key: string]: unknown };
}
export interface UpstreamTimeout {
  connect: number;
  send: number;
  read: number;
}
export interface UpstreamClientTLS {
  client_cert: string;
  client_key: string;
  client_cert_id: string;
  verify: boolean;
}
export interface UpstreamKeepalivePool {
  size: number;
  idle_timeout: number;
  requests: number;
}
export interface UpstreamHealthCheck {
  active: UpstreamHealthCheckActive;
  passive: UpstreamHealthCheckPassive;
}
export interface UpstreamHealthCheckActive {
  type?: 'http' | 'https' | 'tcp';
  timeout?: number;
  concurrency?: number;
  host: string;
  port: number;
  http_path: string;
  https_verify_cert: boolean;
  http_request_headers: Array<string>;
  healthy: UpstreamHealthCheckActiveHealthy;
  unhealthy: UpstreamHealthCheckActiveUnhealthy;
}
export interface UpstreamHealthCheckPassive {
  type: string;
  healthy: UpstreamHealthCheckPassiveHealthy;
  unhealthy: UpstreamHealthCheckPassiveUnhealthy;
}
export interface UpstreamHealthCheckPassiveHealthy {
  http_statuses: Array<number>;
  successes: number;
}
export interface UpstreamHealthCheckPassiveUnhealthy {
  http_statuses: Array<number>;
  http_failures: number;
  tcp_failures: number;
  timeouts: number;
}

export type UpstreamHealthCheckActiveHealthy = {
  interval: number;
} & UpstreamHealthCheckPassiveHealthy;
export type UpstreamHealthCheckActiveUnhealthy = {
  interval: number;
} & UpstreamHealthCheckPassiveUnhealthy;

export interface Upstream {
  id?: string;
  name?: string;
  description?: string;
  labels?: Labels;

  type?: UpstreamBalancer;
  hash_on?: string;
  key?: string;
  checks?: UpstreamHealthCheck;
  nodes?: Array<UpstreamNode>;
  scheme?: UpstreamScheme;
  retries?: number;
  retry_timeout?: number;
  timeout?: UpstreamTimeout;
  tls?: UpstreamClientTLS;
  keepalive_pool?: UpstreamKeepalivePool;
  pass_host?: UpstreamPassHost;
  upstream_host?: string;

  service_name?: string;
  discovery_type?: string;
  discovery_args?: Record<string, unknown>;

  metadata?: ResourceMetadata;
}

export type SSLType = 'server' | 'client';
export interface SSLClientMTLS {
  ca: string;
  depth: number;
  skip_mtls_uri_regex?: Array<string>;
}
export interface SSLCertificate {
  certificate: string;
  key: string;
}
export interface SSL {
  id?: string;
  labels?: Labels;

  type?: SSLType;
  snis: Array<string>;
  certificates: Array<SSLCertificate>;
  client?: SSLClientMTLS;
  ssl_protocols?: Array<string>;

  metadata?: ResourceMetadata;
}

export interface PluginConfig {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  plugins: Plugins;

  metadata?: ResourceMetadata;
}

export type GlobalRule = Record<string, unknown>;

export type PluginMetadata = Record<string, unknown>;

export interface ConsumerCredential {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  type: 'key-auth' | 'basic-auth' | 'jwt-auth' | 'hmac-auth';

  config: Plugin;

  metadata?: ResourceMetadata;
}

export interface Consumer {
  username: string;
  description?: string;
  labels?: Labels;

  plugins?: Plugins;
  credentials?: Array<ConsumerCredential>;
}

export interface ConsumerGroup {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  plugins: Plugins;

  consumers?: Array<Consumer>;

  metadata?: ResourceMetadata;
}

export interface StreamRoute {
  id?: string;
  name: string;
  description?: string;
  labels?: Labels;

  plugins?: Plugins;

  remote_addr?: string;
  server_addr?: string;
  server_port?: number;
  sni?: string;

  metadata?: ResourceMetadata;
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface ResourceMetadata {}

export interface Configuration {
  services?: Array<Service>;
  ssls?: Array<SSL>;
  consumers?: Array<Consumer>;

  // object format resources
  global_rules?: Record<string, GlobalRule>;
  plugin_metadata?: Record<string, PluginMetadata>;

  // internal use only
  routes?: Array<Route>;
  stream_routes?: Array<StreamRoute>;
  consumer_credentials?: Array<ConsumerCredential>;
  upstreams?: Array<Upstream>;
  /* consumer_groups?: Array<ConsumerGroup>;
  plugin_configs?: Array<PluginConfig>; */
}

export type Resource =
  | Route
  | SSL
  | GlobalRule
  | PluginConfig
  | PluginMetadata
  | Consumer
  | ConsumerGroup
  | StreamRoute
  | Service
  | Upstream;
