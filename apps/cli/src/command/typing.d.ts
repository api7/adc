import * as ADCSDK from '@api7/adc-sdk';

export interface GlobalOptions {
  verbose: number;
}

export type BackendOptions = {
  backend: string;
  server: string;
  token: string;
  gatewayGroup: string;

  labelSelector?: Record<string, string>;
  includeResourceType?: Array<ADCSDK.ResourceType>;
  excludeResourceType?: Array<ADCSDK.ResourceType>;
} & GlobalOptions;

export interface KVConfiguration {
  routes?: Record<string, ADCSDK.Route>;
  services?: Record<string, ADCSDK.Service>;
  upstreams?: Record<string, ADCSDK.Upstream>;
  ssls?: Record<string, ADCSDK.SSL>;
  global_rules?: Record<string, ADCSDK.GlobalRule>;
  plugin_configs?: Record<string, ADCSDK.PluginConfig>;
  plugin_metadata?: Record<string, ADCSDK.PluginMetadata>;
  consumers?: Record<string, ADCSDK.Consumer>;
  consumer_groups?: Record<string, ADCSDK.ConsumerGroup>;
  stream_routes?: Record<string, ADCSDK.StreamRoute>;
}
