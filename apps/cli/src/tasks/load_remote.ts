import { BackendAPI7 } from '@api7/adc-backend-api7';
import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';
import { lastValueFrom } from 'rxjs';

import {
  addBackendEventListener,
  filterConfiguration,
  filterResourceType,
} from '../command/utils';

export interface LoadRemoteConfigurationTaskOptions {
  labelSelector?: ADCSDK.BackendOptions['labelSelector'];
  includeResourceType?: Array<ADCSDK.ResourceType>;
  excludeResourceType?: Array<ADCSDK.ResourceType>;
}

export const LoadRemoteConfigurationTask = ({
  labelSelector,
  includeResourceType,
  excludeResourceType,
}: LoadRemoteConfigurationTaskOptions): ListrTask => ({
  title: 'Load remote configuration',
  task: async (ctx, task) => {
    return task.newListr([
      {
        title: 'Fetch all configuration',
        task: async (ctx, task) => {
          const cancel = addBackendEventListener(ctx.backend, task);
          ctx.remote = await lastValueFrom(
            (ctx.backend as ADCSDK.Backend).dump(),
          );
          ctx.remote = structuredClone(ctx.remote); //TODO: refactor this to immer produce
          cancel();
        },
      },
      {
        title: 'Filter configuration resource type',
        enabled: () =>
          //TODO implement API-level resource filtering on APISIX backend
          !(ctx.backend instanceof BackendAPI7) &&
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
