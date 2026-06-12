import * as ADCSDK from '@api7/adc-sdk';
import { existsSync } from 'fs';
import { readFile } from 'fs/promises';
import { globSync } from 'glob';
import YAML from 'js-yaml';
import { ListrTask } from 'listr2';
import path from 'node:path';

import {
  fillLabels,
  filterResourceType,
  mergeKVConfigurations,
  recursiveReplaceEnvVars,
  toConfiguration,
  toKVConfiguration,
} from '../command/utils';

export const LoadLocalConfigurationTask = (
  files: Array<string>,
  labelSelector?: ADCSDK.BackendOptions['labelSelector'],
  includeResourceType?: Array<ADCSDK.ResourceType>,
  excludeResourceType?: Array<ADCSDK.ResourceType>,
): ListrTask<{
  local: ADCSDK.Configuration;
}> => ({
  title: `Load local configuration`,
  task: async (ctx, task) => {
    if (!files || files.length <= 0) files = ['adc.yaml'];

    interface LoadLocalContext {
      configurations: Record<string, ADCSDK.Configuration>;
    }
    const subCtx: LoadLocalContext = {
      configurations: {},
    };
    return task.newListr<LoadLocalContext>(
      [
        // load yaml files
        ...files
          .flatMap((filePath) =>
            globSync(filePath, { nodir: true, absolute: true }),
          )
          .map((filePath): ListrTask<LoadLocalContext> => {
            return {
              title: `Load ${filePath}`,
              task: async (subCtx) => {
                if (!existsSync(filePath))
                  throw new Error(
                    `File ${path.resolve(filePath)} does not exist`,
                  );
                const fileContent =
                  (await readFile(filePath, { encoding: 'utf-8' })) ?? '';

                const ext = path.extname(filePath).toLowerCase();
                const config: ADCSDK.Configuration =
                  ext === '.json'
                    ? (JSON.parse(fileContent) ?? {})
                    : (YAML.load(fileContent) ?? {});

                // Inline external Lua sources referenced by custom plugins and
                // validate the declared name against the source.
                await resolveCustomPluginSources(config, filePath);

                subCtx.configurations[filePath] = config;
              },
            };
          }),
        // merge yaml files
        {
          title: 'Merge local configurations',
          task: async (subCtx) => {
            const localKVConfiguration = mergeKVConfigurations(
              Object.fromEntries(
                Object.entries(subCtx.configurations).map(
                  ([filePath, configuration]) => [
                    filePath,
                    toKVConfiguration(configuration, filePath),
                  ],
                ),
              ),
            );
            ctx.local = toConfiguration(localKVConfiguration);
          },
        },
        {
          title: 'Resolve value variables',
          task: async () => {
            ctx.local = recursiveReplaceEnvVars(ctx.local);
          },
        },
        {
          title: 'Filter configuration resource type',
          enabled: () =>
            includeResourceType?.length > 0 || excludeResourceType?.length > 0,
          task: () => {
            ctx.local = filterResourceType(
              ctx.local,
              includeResourceType,
              excludeResourceType,
            );
          },
        },
        {
          title: 'Filter configuration',
          enabled: !!labelSelector && Object.keys(labelSelector).length > 0,
          task: async () => {
            // Merge label selectors from CLI inputs to each resource
            fillLabels(ctx.local, labelSelector);
          },
        },
      ],
      {
        ctx: subCtx,
      },
    );
  },
});

// Resolve each custom plugin's `path` reference into an inline `content`
// (read relative to the config file), and verify that the declared `name`
// actually appears in the (plaintext) Lua source so a typo is caught locally
// rather than rejected later by the control plane.
const resolveCustomPluginSources = async (
  config: ADCSDK.Configuration,
  configFilePath: string,
) => {
  const customPlugins = config.custom_plugins;
  if (!Array.isArray(customPlugins) || customPlugins.length === 0) return;

  const baseDir = path.dirname(configFilePath);
  const looksLikeLua = (source: string) =>
    /\b(function|return|local)\b/.test(source);

  for (const plugin of customPlugins) {
    if (plugin.path) {
      const sourcePath = path.resolve(baseDir, plugin.path);
      if (!existsSync(sourcePath))
        throw new Error(
          `Custom plugin "${plugin.name}" references a source file that does not exist: ${sourcePath}`,
        );
      plugin.content =
        (await readFile(sourcePath, { encoding: 'utf-8' })) ?? '';
      delete plugin.path;
    }

    // Only validate when the source is plaintext Lua; obfuscated/bytecode
    // uploads will not contain the name literally.
    if (
      plugin.content &&
      looksLikeLua(plugin.content) &&
      !plugin.content.includes(plugin.name)
    )
      throw new Error(
        `Custom plugin name "${plugin.name}" was not found in its Lua source; the declared name must match the plugin's name in the source.`,
      );
  }
};
