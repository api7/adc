import { BackendAPI7 } from '@api7/adc-backend-api7';
import { BackendAPISIX } from '@api7/adc-backend-apisix';
import * as ADCSDK from '@api7/adc-sdk';
import chalk from 'chalk';
import { isObject, mapValues } from 'lodash';
import path from 'node:path';
import pluralize from 'pluralize';

import { KVConfiguration } from './typing';

export const loadBackend = (
  type: string,
  opts: ADCSDK.BackendOptions,
): ADCSDK.Backend => {
  switch (type) {
    case 'api7ee':
      return new BackendAPI7(opts);
    case 'apisix':
    default:
      console.log(chalk.red(`Apache APISIX backend is experimental!`));
      return new BackendAPISIX(opts);
  }
};

// convert the configuration to a key-value pairs keyed by name/username/snis
export const toKVConfiguration = (
  configuration: ADCSDK.Configuration,
  filePath: string,
): KVConfiguration =>
  Object.fromEntries(
    Object.entries(configuration).map(([resourceType, resources]) => {
      if (['global_rules', 'plugin_metadata'].includes(resourceType)) {
        return [resourceType, resources];
      } else if (resourceType === 'ssls') {
        return [
          resourceType,
          Object.fromEntries(
            configuration.ssls.map((item) => [item.snis.join(','), item]),
          ),
        ];
      } else if (resourceType === 'consumers') {
        return [
          resourceType,
          Object.fromEntries(
            configuration.consumers.map((item) => [item.username, item]),
          ),
        ];
      } else if (
        [
          'routes',
          'services',
          'plugin_configs',
          'consumer_groups',
          'plugin_metadata',
          'stream_routes',
          'upstreams',
        ].includes(resourceType) // ensure that you don't convert unexpected keys
      ) {
        return [
          resourceType,
          Object.fromEntries(resources.map((item) => [item.name, item])),
        ];
      } else {
        throw new Error(
          `Configuration file "${path.resolve(
            filePath,
          )}" contains an unknown key "${resourceType}"`,
        );
      }
    }),
  );

// convert key-value pairs to a array configuration
export const toConfiguration = (
  configuration: KVConfiguration,
): ADCSDK.Configuration =>
  Object.fromEntries(
    Object.entries(configuration).map(([resourceType, resources]) => {
      if (['global_rules', 'plugin_metadata'].includes(resourceType))
        return [resourceType, resources];
      return [resourceType, Object.values(resources)];
    }),
  );

/**
 * Merge KVConfiguration and send warnings when there is the same resource name
 * @param configurations Key-value pairs for file name and KVConfiguration
 * @throws Will throw an error if a resource name is duplicated
 */
export const mergeKVConfigurations = (
  configurations: Record<string, KVConfiguration>,
) => {
  const configurationArr = Object.entries(configurations); // [string, KVConfiguration]
  const baseConfiguration = configurationArr.shift()[1];
  configurationArr.forEach(
    ([fileName, configuration]: [string, KVConfiguration]) => {
      Object.entries(configuration).forEach(
        ([resourceType, resources]: [string, Record<string, unknown>]) => {
          Object.keys(resources).forEach((keyName: string) => {
            if (!baseConfiguration[resourceType])
              baseConfiguration[resourceType] = {};
            if (baseConfiguration[resourceType][keyName]) {
              throw new Error(
                `Duplicate ${pluralize.singular(
                  resourceType,
                )} "${keyName}" was found in ${path.resolve(fileName)}`,
              );
            } else {
              baseConfiguration[resourceType][keyName] = resources[keyName];
            }
          });
        },
      );
    },
  );
  return baseConfiguration;
};

export const mergeConfigurations = (
  ...fileContents: Array<ADCSDK.Configuration>
) => {
  const result: ADCSDK.Configuration = {
    services: [],
    ssls: [],
    consumers: [],
    global_rules: {},
    plugin_metadata: {},

    routes: [],
    stream_routes: [],
    /* consumer_groups: [],
    plugin_configs: [],
    upstreams: [], */
  };

  fileContents.forEach((config) => {
    config.services && result.services.push(...config.services);
    config.ssls && result.ssls.push(...config.ssls);
    config.consumers && result.consumers.push(...config.consumers);
    config.global_rules &&
      Object.keys(config.global_rules).forEach((globalRuleName: string) => {
        result.global_rules[globalRuleName] =
          config.global_rules[globalRuleName];
      });
    config.plugin_metadata &&
      Object.keys(config.plugin_metadata).forEach(
        (pluginMetadataName: string) => {
          result.plugin_metadata[pluginMetadataName] =
            config.plugin_metadata[pluginMetadataName];
        },
      );

    config.routes && result.routes.push(...config.routes);
    config.stream_routes && result.stream_routes.push(...config.stream_routes);
    /* config.consumer_groups &&
      result.consumer_groups.push(...config.consumer_groups);
    config.plugin_configs &&
      result.plugin_configs.push(...config.plugin_configs);
    config.upstreams && result.upstreams.push(...config.upstreams); */
  });

  return result;
};

export const filterConfiguration = (
  configuration: ADCSDK.Configuration,
  rules: Record<string, string>,
): [ADCSDK.Configuration, ADCSDK.Configuration] => {
  const removed: ADCSDK.Configuration = {};
  Object.keys(configuration).forEach((resourceType) => {
    if (resourceType === 'plugin_metadata' || resourceType === 'global_rules')
      return;
    const result = labelFilter(configuration[resourceType], rules);
    configuration[resourceType] = result.filtered;
    removed[resourceType] = result.removed;
  });

  return [configuration, removed];
};

const labelFilter = <T extends ADCSDK.Event['newValue']>(
  resources: Array<T> = [],
  rules: Record<string, string> = {},
) => {
  const filtered = resources.filter((resource) => {
    return Object.entries(rules).every(
      ([key, value]) =>
        resource?.labels &&
        resource?.labels[key] &&
        resource?.labels[key] === value,
    );
  });

  return {
    filtered,
    removed: resources.filter((resource) => !filtered.includes(resource)),
  };
};

export const fillLabels = (
  configuration: ADCSDK.Configuration,
  rules: Record<string, string>,
) => {
  const assignSelector = (labels: object) => Object.assign(labels ?? {}, rules);

  for (const resourceType in configuration) {
    if (['global_rules', 'plugin_metadata'].includes(resourceType)) continue;

    (configuration[resourceType] as Array<ADCSDK.Resource>).forEach(
      (resource) => {
        resource.labels = assignSelector(resource.labels as object);

        // Process the nested resources
        if (resourceType === 'services') {
          const sub = resource as ADCSDK.Service;
          sub?.routes?.forEach((item) => {
            item.labels = assignSelector(item.labels);
          });
          sub?.stream_routes?.forEach((item) => {
            item.labels = assignSelector(item.labels);
          });
        } else if (resourceType === 'consumer_groups') {
          const sub = resource as ADCSDK.ConsumerGroup;
          sub?.consumers?.forEach((item) => {
            item.labels = assignSelector(item.labels);
          });
        }
      },
    );
  }

  return configuration;
};

export const filterResourceType = (
  config: ADCSDK.Configuration,
  includes: Array<string>,
  excludes: Array<string>,
) => {
  const key = (item) =>
    item !== ADCSDK.ResourceType.PLUGIN_METADATA ? item.slice(0, -1) : item;
  return Object.fromEntries(
    Object.entries(config).filter(([resourceType]) => {
      return includes
        ? includes.includes(key(resourceType))
        : !excludes.includes(key(resourceType));
    }),
  );
};

export const recursiveRemoveMetadataField = (c: ADCSDK.Configuration) => {
  const removeMetadata = (obj: object) => {
    if ('metadata' in obj) delete obj.metadata;
  };
  Object.entries(c).forEach(([key, value]) => {
    if (['global_rules', 'plugin_metadata'].includes(key)) return;
    if (Array.isArray(value))
      value.forEach((item) => {
        removeMetadata(item);
        if (key === 'services') {
          if ('routes' in item && Array.isArray(item.routes))
            item.routes.forEach((r) => removeMetadata(r));
          if ('stream_routes' in item && Array.isArray(item.stream_routes))
            item.stream_routes.forEach((r) => removeMetadata(r));
        } else if (key === 'consumer_groups') {
          if ('consumers' in item && Array.isArray(item.consumers))
            item.consumers.forEach((c) => removeMetadata(c));
        } else if (key === 'consumers') {
          if ('credentials' in item && Array.isArray(item.credentials))
            item.credentials.forEach((c) => removeMetadata(c));
        }
      });
  });
};

export const recursiveReplaceEnvVars = (
  c: ADCSDK.Configuration,
  dataSource = process.env,
): ADCSDK.Configuration => {
  const envVarRegex = /\$\{(\w+)\}/g;
  const replaceValue = (value: unknown): unknown => {
    if (typeof value === 'string')
      return value.replace(
        envVarRegex,
        (_, envVar) => dataSource?.[envVar] || '',
      );

    return value;
  };

  const recurseReplace = (value: unknown): unknown =>
    isObject(value) && !Array.isArray(value)
      ? mapValues(value, recurseReplace)
      : Array.isArray(value)
        ? value.map(recurseReplace)
        : replaceValue(value);

  return mapValues(c, recurseReplace) as ADCSDK.Configuration;
};

export const configurePluralize = () => {
  pluralize.addIrregularRule('route', 'routes');
  pluralize.addIrregularRule('service', 'services');
  pluralize.addIrregularRule('upstream', 'upstreams');
  pluralize.addIrregularRule('ssl', 'ssls');
  pluralize.addIrregularRule('global_rule', 'global_rules');
  pluralize.addIrregularRule('plugin_config', 'plugin_configs');
  pluralize.addIrregularRule('plugin_metadata', 'plugin_metadata');
  pluralize.addIrregularRule('consumer', 'consumers');
  pluralize.addIrregularRule('consumer_group', 'consumer_groups');
  pluralize.addIrregularRule('stream_route', 'stream_routes');
};
