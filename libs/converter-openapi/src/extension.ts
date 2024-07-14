import { isEmpty } from 'lodash';

export enum ExtKey {
  NAME = 'x-adc-name',
  LABELS = 'x-adc-labels',
  PLUGINS = 'x-adc-plugins',
  PLUGIN_PREFIX = 'x-adc-plugin-',
  SERVICE_DEFAULTS = 'x-adc-service-defaults',
  UPSTREAM_DEFAULTS = 'x-adc-upstream-defaults',
  UPSTREAM_NODE_DEFAULTS = 'x-adc-upstream-node-defaults',
  ROUTE_DEFAULTS = 'x-adc-route-defaults',
}

export const parseExtPlugins = (context: object) => {
  const separatePlugins = Object.fromEntries(
    Object.entries(context)
      .filter(
        ([key]) => key.startsWith(ExtKey.PLUGIN_PREFIX), // x-adc-plugin-acl
      )
      .map(([key, plugin]) => [key.replace(ExtKey.PLUGIN_PREFIX, ''), plugin]),
  );
  const plugins = { ...context[ExtKey.PLUGINS], ...separatePlugins };
  return !isEmpty(plugins) ? plugins : undefined;
};
