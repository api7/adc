import { AxiosResponse } from 'axios';
import { curry } from 'lodash-es';
import { Subject } from 'rxjs';

import * as ADCSDK from '..';

export const MANAGED_BY_LABEL_KEY = 'managed-by';
export const MANAGED_BY_LABEL_VALUE = 'adc';

// Must run after the differ has already produced event.diff — mutating labels
// here never feeds back into the diff patch, otherwise every resource would
// show a spurious label change on the first sync (remote doesn't have it yet).
export const injectManagedByLabel = (
  events: Array<ADCSDK.Event>,
  enabled = true,
): Array<ADCSDK.Event> => {
  if (!enabled) return events;
  // GLOBAL_RULE / PLUGIN_METADATA have no `labels` field in their schema, skip them.
  // Built lazily (not at module scope) to avoid resolving ADCSDK.ResourceType
  // before the circular `..` barrel import has finished initializing.
  const unlabelableResourceTypes = new Set<ADCSDK.ResourceType>([
    ADCSDK.ResourceType.GLOBAL_RULE,
    ADCSDK.ResourceType.PLUGIN_METADATA,
  ]);
  events.forEach((event) => {
    if (
      (event.type !== ADCSDK.EventType.CREATE &&
        event.type !== ADCSDK.EventType.UPDATE) ||
      !event.newValue ||
      unlabelableResourceTypes.has(event.resourceType)
    )
      return;
    (event.newValue as { labels?: ADCSDK.Labels }).labels = {
      ...(event.newValue as { labels?: ADCSDK.Labels }).labels,
      [MANAGED_BY_LABEL_KEY]: MANAGED_BY_LABEL_VALUE,
    };
  });
  return events;
};

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
