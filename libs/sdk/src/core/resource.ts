export enum ResourceType {
  ROUTE = 'route',
  SERVICE = 'service',
  UPSTREAM = 'upstream',
  SSL = 'ssl',
  GLOBAL_RULE = 'global_rule',
  PLUGIN_CONFIG = 'plugin_config',
  PLUGIN_METADATA = 'plugin_metadata',
  CONSUMER = 'consumer',
  CONSUMER_GROUP = 'consumer_group',
  CONSUMER_CREDENTIAL = 'consumer_credential',
  STREAM_ROUTE = 'stream_route',

  // internal use only
  INTERNAL_STREAM_SERVICE = 'stream_service',
}
