import { DifferV3 } from '@api7/adc-differ';
import * as ADCSDK from '@api7/adc-sdk';
import { ListrTask } from 'listr2';

import { TaskContext } from '../command/diff.command';
import { ListrOutputLogger } from '../utils/listr';

export const DiffResourceTask = (
  printSummary = false,
  persistentSummary = false,
): ListrTask<TaskContext> => ({
  title: 'Diff configuration',
  task: async (ctx, task) => {
    const backend = ctx.backend;
    const defaultValue = await backend.defaultValue();
    const logger = new ListrOutputLogger(task);
    ctx.diff = DifferV3.diff(
      ctx.local,
      ctx.remote,
      defaultValue,
      undefined,
      logger,
    );

    if (printSummary) {
      task.output = '';
      let [created, updated, deleted] = [0, 0, 0];
      ctx.diff.forEach((event) => {
        switch (event.type) {
          case ADCSDK.EventType.CREATE:
            task.output += `create ${event.resourceType}: "${event.resourceName}"\n`;
            created++;
            break;
          case ADCSDK.EventType.DELETE:
            task.output += `delete ${event.resourceType}: "${event.resourceName}"\n`;
            deleted++;
            break;
          case ADCSDK.EventType.UPDATE:
            task.output += `update ${event.resourceType}: "${event.resourceName}"\n`;
            updated++;
            break;
        }
      });
      task.output += `Summary: ${created} will be created, ${updated} will be updated, ${deleted} will be deleted`;
    }
  },
});
