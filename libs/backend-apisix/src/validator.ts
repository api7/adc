import * as ADCSDK from '@api7/adc-sdk';
import axios, { type AxiosInstance, type AxiosRequestConfig } from 'axios';
import { Subject } from 'rxjs';

import { FromADC } from './transformer';
import * as typing from './typing';

export interface ValidatorOptions {
  client: AxiosInstance;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  requestConfig?: AxiosRequestConfig;
}

interface ValidateRequestBody {
  routes: Array<typing.Route>;
  services: Array<typing.Service>;
  consumers: Array<typing.Consumer>;
  ssls: Array<typing.SSL>;
  global_rules: Array<typing.GlobalRule>;
  stream_routes: Array<typing.StreamRoute>;
  plugin_metadata: Array<Record<string, unknown>>;
  upstreams: Array<typing.Upstream>;
}

export class Validator extends ADCSDK.backend.BackendEventSource {
  private readonly client: AxiosInstance;
  private readonly fromADC = new FromADC();

  constructor(private readonly opts: ValidatorOptions) {
    super();
    this.client = opts.client;
    this.subject = opts.eventSubject;
  }

  public async validate(
    events: Array<ADCSDK.Event>,
  ): Promise<ADCSDK.BackendValidateResult> {
    const { body, nameIndex } = this.buildRequestBody(events);

    try {
      const resp = await this.client.post(
        '/apisix/admin/configs/validate',
        body,
        this.opts.requestConfig,
      );
      this.subject.next({
        type: ADCSDK.BackendEventType.AXIOS_DEBUG,
        event: { response: resp, description: 'Validate configuration' },
      });
      return { success: true, errors: [] };
    } catch (error) {
      if (axios.isAxiosError(error) && error.response?.status === 400) {
        this.subject.next({
          type: ADCSDK.BackendEventType.AXIOS_DEBUG,
          event: {
            response: error.response,
            description: 'Validate configuration (failed)',
          },
        });
        const data = error.response.data;
        const errors: ADCSDK.BackendValidationError[] = (
          data?.errors ?? []
        ).map((e: ADCSDK.BackendValidationError) => {
          const name = nameIndex[e.resource_type]?.[e.index];
          return name ? { ...e, resource_name: name } : e;
        });
        return {
          success: false,
          errorMessage: data?.error_msg,
          errors,
        };
      }
      throw error;
    }
  }

  private buildRequestBody(events: Array<ADCSDK.Event>): {
    body: ValidateRequestBody;
    nameIndex: Record<string, string[]>;
  } {
    const body: ValidateRequestBody = {
      routes: [],
      services: [],
      consumers: [],
      ssls: [],
      global_rules: [],
      stream_routes: [],
      plugin_metadata: [],
      upstreams: [],
    };
    const nameIndex: Record<string, string[]> = {
      routes: [],
      services: [],
      consumers: [],
      ssls: [],
      global_rules: [],
      stream_routes: [],
      plugin_metadata: [],
      upstreams: [],
    };

    const flat = events.filter(
      (e) =>
        e.type === ADCSDK.EventType.CREATE ||
        e.type === ADCSDK.EventType.UPDATE,
    );

    for (const event of flat) {
      switch (event.resourceType) {
        case ADCSDK.ResourceType.SERVICE: {
          (event.newValue as ADCSDK.Service).id = event.resourceId;
          const [service, upstream] = this.fromADC.transformService(
            event.newValue as ADCSDK.Service,
          );
          body.services.push(service);
          nameIndex.services.push(event.resourceName);
          if (upstream) {
            body.upstreams.push(upstream);
            nameIndex.upstreams.push(event.resourceName);
          }
          break;
        }
        case ADCSDK.ResourceType.ROUTE: {
          (event.newValue as ADCSDK.Route).id = event.resourceId;
          body.routes.push(
            this.fromADC.transformRoute(
              event.newValue as ADCSDK.Route,
              event.parentId!,
            ),
          );
          nameIndex.routes.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.STREAM_ROUTE: {
          (event.newValue as ADCSDK.StreamRoute).id = event.resourceId;
          body.stream_routes.push(
            this.fromADC.transformStreamRoute(
              event.newValue as ADCSDK.StreamRoute,
              event.parentId!,
            ),
          );
          nameIndex.stream_routes.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.CONSUMER: {
          body.consumers.push(
            this.fromADC.transformConsumer(event.newValue as ADCSDK.Consumer),
          );
          nameIndex.consumers.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.SSL: {
          (event.newValue as ADCSDK.SSL).id = event.resourceId;
          body.ssls.push(
            this.fromADC.transformSSL(event.newValue as ADCSDK.SSL),
          );
          nameIndex.ssls.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.GLOBAL_RULE: {
          body.global_rules.push({
            plugins: { [event.resourceId]: event.newValue },
          } as unknown as typing.GlobalRule);
          nameIndex.global_rules.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.PLUGIN_METADATA: {
          body.plugin_metadata.push({
            id: event.resourceId,
            ...ADCSDK.utils.recursiveOmitUndefined(
              event.newValue as Record<string, unknown>,
            ),
          });
          nameIndex.plugin_metadata.push(event.resourceName);
          break;
        }
      }
    }
    return { body, nameIndex };
  }
}
