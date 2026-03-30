import { AxiosResponse } from 'axios';
import { curry } from 'lodash-es';
import { Subject } from 'rxjs';

import * as ADCSDK from '..';

export class BackendEventSource {
  protected subject!: Subject<ADCSDK.BackendEvent>;
  private _innerSubject: Subject<{
    name: string;
    event: ADCSDK.BackendEvent;
  }> = new Subject();
  protected _taskEvents: Map<string, Array<ADCSDK.BackendEvent>> = new Map();

  constructor() {
    this._innerSubject.subscribe({
      next: ({ name, event }) => {
        if (!this._taskEvents.has(name)) this._taskEvents.set(name, []);
        this._taskEvents.get(name)?.push(event);

        if (event.type === 'TASK_DONE')
          this._taskEvents
            .get(name)
            ?.forEach((event) => this.subject.next(event));
      },
    });
  }

  protected getLogger: (
    name: string,
  ) => (event: ADCSDK.BackendEvent) => void = curry(
    (name: string, event: ADCSDK.BackendEvent) => {
      this._innerSubject.next({ name, event });
    },
  );

  protected taskStateEvent: (
    name: string,
  ) => (
    type:
      | typeof ADCSDK.BackendEventType.TASK_START
      | typeof ADCSDK.BackendEventType.TASK_DONE,
  ) => ADCSDK.BackendEvent = curry(
    (
      name: string,
      type:
        | typeof ADCSDK.BackendEventType.TASK_START
        | typeof ADCSDK.BackendEventType.TASK_DONE,
    ): ADCSDK.BackendEvent => ({
      type,
      event: { name },
    }),
  );

  protected debugLogEvent = (
    response: AxiosResponse,
    description?: string,
  ): ADCSDK.BackendEvent => ({
    type: ADCSDK.BackendEventType.AXIOS_DEBUG,
    event: { response, description },
  });
}
