import { BackendAPI7 } from '@api7/adc-backend-api7';
import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';

import { filterConfiguration, filterResourceType } from '../command/utils';

export interface LoadRemoteConfigurationTaskOptions {
  backend: ADCSDK.Backend;
  labelSelector?: ADCSDK.BackendOptions['labelSelector'];
  includeResourceType?: Array<ADCSDK.ResourceType>;
  excludeResourceType?: Array<ADCSDK.ResourceType>;
}

export const LoadRemoteConfigurationTask = ({
  backend,
  labelSelector,
  includeResourceType,
  excludeResourceType,
}: LoadRemoteConfigurationTaskOptions): ListrTask => ({
  title: 'Load remote configuration',
  task: async (ctx, task) => {
    return task.newListr([
      {
        title: 'Fetch all configuration',
        task: async () => await backend.dump(),
      },
      {
        title: 'Filter configuration resource type',
        enabled: () =>
          //TODO implement API-level resource filtering on APISIX backend
          !(backend instanceof BackendAPI7) &&
          (includeResourceType?.length > 0 || excludeResourceType?.length > 0),
        task: () => {
          ctx.remote = filterResourceType(
            ctx.remote,
            includeResourceType,
            excludeResourceType,
          );
        },
      },
      {
        title: 'Filter remote configuration',
        enabled: !!labelSelector,
        task: (ctx) => {
          [ctx.remote] = filterConfiguration(ctx.remote, labelSelector);
        },
      },
    ]);
  },
});
