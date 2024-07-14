import * as ADCSDK from '@api7/adc-sdk';
import {
  ListrRenderer,
  ListrTaskEventType,
  ListrTaskObject,
  ListrTaskState,
} from 'listr2';
import { attempt, isError } from 'lodash';

type SignaleRendererTask<T extends object = object> = ListrTaskObject<
  T,
  typeof SignaleRenderer,
  typeof SignaleRenderer
>;

export interface SignaleRendererOptions {
  verbose?: number;
  scope?: Array<string>;
}

export interface SignaleRendererOutput {
  type: string;
  messages: Array<unknown>;
}

export class SignaleRenderer implements ListrRenderer {
  public static nonTTY = true;
  public static rendererOptions: SignaleRendererOptions = {
    verbose: 1,
    scope: ['ADC'],
  };
  public static rendererTaskOptions: never;

  // get tasks to be rendered and options of the renderer from the parent
  constructor(
    private readonly tasks: SignaleRendererTask[],
    private options: SignaleRendererOptions,
  ) {
    this.options = {
      ...SignaleRenderer.rendererOptions,
      ...this.options,
    };
  }

  // implement custom logic for render functionality
  public render() {
    this.renderer(this.tasks);
  }

  private renderer(tasks: SignaleRendererTask[]) {
    tasks.forEach((task) => {
      const rendererOptions = {
        ...this.options,
        ...task.rendererOptions,
      };

      task.on(ListrTaskEventType.SUBTASK, (subTasks) => {
        return this.renderer(subTasks);
      });

      task.on(ListrTaskEventType.STATE, (state) => {
        if (!task.hasTitle()) return;

        if (state === ListrTaskState.STARTED) {
          rendererOptions?.verbose > 0 &&
            ADCSDK.utils.getLogger(rendererOptions?.scope).start(task.title);
        }
        if (state === ListrTaskState.COMPLETED) {
          rendererOptions?.verbose > 0 &&
            ADCSDK.utils.getLogger(rendererOptions?.scope).success(task.title);
        }
        if (state === ListrTaskState.SKIPPED) {
          rendererOptions?.verbose > 0 &&
            ADCSDK.utils
              .getLogger(rendererOptions?.scope)
              .info(
                `${task.title} is skipped${task.message.skip ? `: ${task.message.skip}` : ''}`,
              );
        }
        if (state === ListrTaskState.FAILED) {
          rendererOptions?.verbose > 0 &&
            ADCSDK.utils.getLogger(rendererOptions?.scope).error(task.title);
        }
      });

      task.on(ListrTaskEventType.OUTPUT, (str) => {
        const output = attempt(JSON.parse, str) as SignaleRendererOutput;
        if (isError(output)) return;
        if (!output.type || !output.messages) return;

        switch (output.type) {
          case 'debug': {
            if (output?.messages && rendererOptions?.verbose === 2) {
              ADCSDK.utils
                .getLogger(rendererOptions?.scope)
                .debug(output.messages.join(''));
            }
            break;
          }
        }
      });
    });
  }

  public end(err?: Error) {
    if (err) {
      ADCSDK.utils.getLogger().fatal(err);
    } else {
      this.options.verbose > 0 &&
        ADCSDK.utils.getLogger().star('All is well, see you next time!');
    }
  }
}
