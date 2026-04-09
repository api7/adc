import type {
  Configuration,
  Plugins,
  PluginSchemaEntry,
  PluginSchemaMap,
} from '@api7/adc-sdk';
import Ajv from 'ajv-draft-04';

import type { LintError } from './types';

const createAjv = () => new Ajv({ allErrors: true, strict: false });

const validateSinglePlugin = (
  ajv: Ajv,
  pluginName: string,
  pluginConfig: Record<string, unknown>,
  schema: PluginSchemaEntry,
  path: Array<string | number>,
  schemaType: 'configSchema' | 'consumerSchema' | 'metadataSchema',
): LintError[] => {
  const errors: LintError[] = [];
  const jsonSchema = schema[schemaType];
  if (!jsonSchema) return errors;

  const validate = ajv.compile(jsonSchema);
  if (!validate(pluginConfig)) {
    for (const err of validate.errors ?? []) {
      const dataPath = err.instancePath
        ? err.instancePath
            .split('/')
            .filter(Boolean)
            .map((seg) => (/^\d+$/.test(seg) ? Number(seg) : seg))
        : [];
      errors.push({
        path: [...path, pluginName, ...dataPath],
        message: err.message ?? 'Validation failed',
        code: 'plugin_schema_violation',
      });
    }
  }
  return errors;
};

const validatePluginsMap = (
  ajv: Ajv,
  plugins: Plugins | undefined,
  schemas: PluginSchemaMap,
  basePath: Array<string | number>,
  schemaType: 'configSchema' | 'consumerSchema' | 'metadataSchema',
): LintError[] => {
  if (!plugins) return [];
  const errors: LintError[] = [];

  for (const [pluginName, pluginConfig] of Object.entries(plugins)) {
    const schema = schemas[pluginName];
    if (!schema) {
      errors.push({
        path: [...basePath, pluginName],
        message: `Unknown plugin "${pluginName}"`,
        code: 'unknown_plugin',
      });
      continue;
    }
    errors.push(
      ...validateSinglePlugin(
        ajv,
        pluginName,
        pluginConfig as Record<string, unknown>,
        schema,
        basePath,
        schemaType,
      ),
    );
  }
  return errors;
};

export const validatePlugins = (
  config: Configuration,
  schemas: PluginSchemaMap,
): LintError[] => {
  const ajv = createAjv();
  const errors: LintError[] = [];

  // Validate service-level and nested route/stream_route plugins
  config.services?.forEach((service, si) => {
    errors.push(
      ...validatePluginsMap(
        ajv,
        service.plugins,
        schemas,
        ['services', si, 'plugins'],
        'configSchema',
      ),
    );

    service.routes?.forEach((route, ri) => {
      errors.push(
        ...validatePluginsMap(
          ajv,
          route.plugins,
          schemas,
          ['services', si, 'routes', ri, 'plugins'],
          'configSchema',
        ),
      );
    });

    service.stream_routes?.forEach((streamRoute, sri) => {
      errors.push(
        ...validatePluginsMap(
          ajv,
          streamRoute.plugins,
          schemas,
          ['services', si, 'stream_routes', sri, 'plugins'],
          'configSchema',
        ),
      );
    });
  });

  // Validate consumer plugins and credential configs
  config.consumers?.forEach((consumer, ci) => {
    errors.push(
      ...validatePluginsMap(
        ajv,
        consumer.plugins,
        schemas,
        ['consumers', ci, 'plugins'],
        'configSchema',
      ),
    );

    consumer.credentials?.forEach((credential, cri) => {
      const pluginName = credential.type;
      const schema = schemas[pluginName];
      if (schema) {
        errors.push(
          ...validateSinglePlugin(
            ajv,
            pluginName,
            credential.config as Record<string, unknown>,
            schema,
            ['consumers', ci, 'credentials', cri, 'config'],
            'consumerSchema',
          ),
        );
      }
    });
  });

  // Validate consumer group plugins
  config.consumer_groups?.forEach((group, gi) => {
    errors.push(
      ...validatePluginsMap(
        ajv,
        group.plugins,
        schemas,
        ['consumer_groups', gi, 'plugins'],
        'configSchema',
      ),
    );
  });

  // Validate global rules
  if (config.global_rules) {
    errors.push(
      ...validatePluginsMap(
        ajv,
        config.global_rules,
        schemas,
        ['global_rules'],
        'configSchema',
      ),
    );
  }

  // Validate plugin metadata
  if (config.plugin_metadata) {
    errors.push(
      ...validatePluginsMap(
        ajv,
        config.plugin_metadata,
        schemas,
        ['plugin_metadata'],
        'metadataSchema',
      ),
    );
  }

  return errors;
};
