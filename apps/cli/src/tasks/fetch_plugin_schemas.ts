import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';

export const FetchPluginSchemasTask = (): ListrTask<{
  backend: ADCSDK.Backend;
  pluginSchemas?: ADCSDK.PluginSchemaMap;
}> => ({
  title: 'Fetch plugin schemas',
  enabled: (ctx) => !!ctx.backend?.fetchPluginSchemas,
  task: async (ctx) => {
    const { fetchPluginSchemas } = ctx.backend;
    if (fetchPluginSchemas) {
      ctx.pluginSchemas = await fetchPluginSchemas();
    }
  },
});
