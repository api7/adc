import { LogEntry, LogEntryOptions, Logger } from '@api7/adc-sdk';
import axios, { AxiosResponse } from 'axios';
import {
  ListrRenderer,
  ListrTaskEventType,
  ListrTaskObject,
  ListrTaskState,
  ListrTaskWrapper,
} from 'listr2';
import { attempt, isError, isObject } from 'lodash';
import { Signale } from 'signale';

type SignaleRendererTask<T extends object = object> = ListrTaskObject<
  T,
  typeof SignaleRenderer,
  typeof SignaleRenderer
>;

export interface SignaleRendererOptions {
  verbose?: number;
  scope?: Array<string>;
}

export const SignaleRendererOutputType = {
  DEBUG: 'DEBUG',
  LISTR_TASK_START: 'LISTR_TASK_START',
  LISTR_TASK_COMPLETE: 'LISTR_TASK_COMPLETE',
} as const;

export interface SignaleRendererOutput {
  type: keyof typeof SignaleRendererOutputType;
  scope?: Array<string>;
  messages: Array<unknown>;
}

export class SignaleRenderer implements ListrRenderer {
  public static nonTTY = true;
  public static rendererOptions: SignaleRendererOptions = {
    verbose: 1,
    scope: ['ADC'],
  };
  public static rendererTaskOptions: never;

  private readonly logger = new Signale({
    config: { displayTimestamp: true },
  });

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

  public end(err?: Error) {
    if (err) {
      this.getScopedLogger().fatal(err);
    } else {
      this.options.verbose > 0 &&
        this.getScopedLogger().star('All is well, see you next time!');
    }
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
            this.getScopedLogger(rendererOptions).start(task.title);
        }
        if (state === ListrTaskState.COMPLETED) {
          rendererOptions?.verbose > 0 &&
            this.getScopedLogger(rendererOptions).success(task.title);
        }
        if (state === ListrTaskState.SKIPPED) {
          rendererOptions?.verbose > 0 &&
            this.getScopedLogger(rendererOptions).info(
              `${task.title} is skipped${task.message.skip ? `: ${task.message.skip}` : ''}`,
            );
        }
        if (state === ListrTaskState.FAILED) {
          rendererOptions?.verbose > 0 &&
            this.getScopedLogger(rendererOptions).error(task.title);
        }
      });

      task.on(ListrTaskEventType.OUTPUT, (str) => {
        if (rendererOptions?.verbose <= 0) return;
        const output = attempt(JSON.parse, str) as SignaleRendererOutput;
        if (isError(output)) return;
        if (!output.type || !output.messages) return;

        switch (output.type) {
          case SignaleRendererOutputType.LISTR_TASK_START: {
            this.getScopedLogger({
              ...rendererOptions,
              scope: output.scope ?? rendererOptions.scope,
            }).start(output.messages.join(''));
            break;
          }
          case SignaleRendererOutputType.LISTR_TASK_COMPLETE: {
            this.getScopedLogger({
              ...rendererOptions,
              scope: output.scope ?? rendererOptions.scope,
            }).success(output.messages.join(''));
            break;
          }
          case SignaleRendererOutputType.DEBUG: {
            if (output?.messages && rendererOptions?.verbose === 2) {
              this.getScopedLogger({
                ...rendererOptions,
                scope: output.scope ?? rendererOptions.scope,
              }).debug(output.messages.join(''));
            }
            break;
          }
        }
      });
    });
  }

  private getScopedLogger(opts?: SignaleRendererOptions) {
    return this.logger.scope(...(opts?.scope ?? ['ADC']));
  }
}

export class ListrOutputLogger implements Logger {
  constructor(
    private readonly task: ListrTaskWrapper<
      unknown,
      typeof SignaleRenderer,
      any
    >,
  ) {}

  log(message: string): void {
    this.task.output = message;
  }

  debug({ message, ...kvs }: LogEntry, opts: LogEntryOptions): void {
    if (opts?.showLogEntry && !opts?.showLogEntry({ message, ...kvs })) return;

    this.task.output = JSON.stringify({
      type: SignaleRendererOutputType.DEBUG,
      messages: [
        `${message}\n`,
        Object.entries(kvs)
          .map(([k, v]) => `${k}: ${isObject(v) ? JSON.stringify(v) : v}`)
          .join('\n'),
      ],
    } satisfies SignaleRendererOutput);
  }
}
