import { AxiosResponse } from 'axios';
import { curry } from 'lodash';
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

  protected getLogger = curry((name: string, event: ADCSDK.BackendEvent) => {
    this._innerSubject.next({ name, event });
  });

  protected taskStateEvent = curry(
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
