import * as ADCSDK from '@api7/adc-sdk';
import axios, { type AxiosInstance } from 'axios';
import { Subject } from 'rxjs';

import { FromADC } from './transformer';
import * as typing from './typing';

export interface ValidatorOptions {
  client: AxiosInstance;
  eventSubject: Subject<ADCSDK.BackendEvent>;
  gatewayGroupId?: string;
}

interface ValidateRequestBody {
  routes?: Array<typing.Route>;
  services?: Array<typing.Service>;
  consumers?: Array<typing.Consumer>;
  ssls?: Array<typing.SSL>;
  global_rules?: Array<typing.GlobalRule>;
  stream_routes?: Array<typing.StreamRoute>;
  plugin_metadata?: Array<Record<string, unknown>>;
  consumer_groups?: Array<Record<string, unknown>>;
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
        { params: { gateway_group_id: this.opts.gatewayGroupId } },
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
        const errors: ADCSDK.BackendValidationError[] = (data?.errors ?? []).map(
          (e: ADCSDK.BackendValidationError) => {
            const name = nameIndex[e.resource_type]?.[e.index];
            return name ? { ...e, resource_name: name } : e;
          },
        );
        return {
          success: false,
          errorMessage: data?.error_msg,
          errors,
        };
      }
      throw error;
    }
  }

  private flattenEvents(events: Array<ADCSDK.Event>): Array<ADCSDK.Event> {
    const flat: Array<ADCSDK.Event> = [];
    for (const event of events) {
      if (event.type !== ADCSDK.EventType.ONLY_SUB_EVENTS) {
        flat.push(event);
      }
      if (event.subEvents?.length) {
        flat.push(...this.flattenEvents(event.subEvents));
      }
    }
    return flat;
  }

  private buildRequestBody(events: Array<ADCSDK.Event>): {
    body: ValidateRequestBody;
    nameIndex: Record<string, string[]>;
  } {
    const body: ValidateRequestBody = {};
    const nameIndex: Record<string, string[]> = {};

    const flat = this.flattenEvents(events).filter(
      (e) =>
        e.type === ADCSDK.EventType.CREATE ||
        e.type === ADCSDK.EventType.UPDATE,
    );

    const services: Array<typing.Service> = [];
    const serviceNames: string[] = [];
    const routes: Array<typing.Route> = [];
    const routeNames: string[] = [];
    const streamRoutes: Array<typing.StreamRoute> = [];
    const streamRouteNames: string[] = [];
    const consumers: Array<typing.Consumer> = [];
    const consumerNames: string[] = [];
    const ssls: Array<typing.SSL> = [];
    const sslNames: string[] = [];
    const globalRules: Array<typing.GlobalRule> = [];
    const globalRuleNames: string[] = [];
    const pluginMetadata: Array<Record<string, unknown>> = [];
    const pluginMetadataNames: string[] = [];
    const consumerGroups: Array<Record<string, unknown>> = [];
    const consumerGroupNames: string[] = [];

    for (const event of flat) {
      switch (event.resourceType) {
        case ADCSDK.ResourceType.SERVICE: {
          (event.newValue as ADCSDK.Service).id = event.resourceId;
          services.push(
            this.fromADC.transformService(event.newValue as ADCSDK.Service),
          );
          serviceNames.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.ROUTE: {
          (event.newValue as ADCSDK.Route).id = event.resourceId;
          routes.push(
            this.fromADC.transformRoute(
              event.newValue as ADCSDK.Route,
              event.parentId!,
            ),
          );
          routeNames.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.STREAM_ROUTE: {
          (event.newValue as ADCSDK.StreamRoute).id = event.resourceId;
          streamRoutes.push(
            this.fromADC.transformStreamRoute(
              event.newValue as ADCSDK.StreamRoute,
              event.parentId!,
            ),
          );
          streamRouteNames.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.CONSUMER: {
          consumers.push(
            this.fromADC.transformConsumer(event.newValue as ADCSDK.Consumer),
          );
          consumerNames.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.SSL: {
          (event.newValue as ADCSDK.SSL).id = event.resourceId;
          ssls.push(
            this.fromADC.transformSSL(event.newValue as ADCSDK.SSL),
          );
          sslNames.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.GLOBAL_RULE: {
          globalRules.push({
            plugins: { [event.resourceId]: event.newValue },
          } as unknown as typing.GlobalRule);
          globalRuleNames.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.PLUGIN_METADATA: {
          pluginMetadata.push({
            id: event.resourceId,
            ...ADCSDK.utils.recursiveOmitUndefined(
              event.newValue as Record<string, unknown>,
            ),
          });
          pluginMetadataNames.push(event.resourceName);
          break;
        }
        case ADCSDK.ResourceType.CONSUMER_GROUP: {
          const cg = event.newValue as ADCSDK.ConsumerGroup;
          consumerGroups.push(
            ADCSDK.utils.recursiveOmitUndefined({
              id: event.resourceId,
              name: cg.name,
              desc: cg.description,
              labels: cg.labels,
              plugins: cg.plugins,
            }) as unknown as Record<string, unknown>,
          );
          consumerGroupNames.push(event.resourceName);
          break;
        }
      }
    }

    if (services.length) {
      body.services = services;
      nameIndex.services = serviceNames;
    }
    if (routes.length) {
      body.routes = routes;
      nameIndex.routes = routeNames;
    }
    if (streamRoutes.length) {
      body.stream_routes = streamRoutes;
      nameIndex.stream_routes = streamRouteNames;
    }
    if (consumers.length) {
      body.consumers = consumers;
      nameIndex.consumers = consumerNames;
    }
    if (ssls.length) {
      body.ssls = ssls;
      nameIndex.ssls = sslNames;
    }
    if (globalRules.length) {
      body.global_rules = globalRules;
      nameIndex.global_rules = globalRuleNames;
    }
    if (pluginMetadata.length) {
      body.plugin_metadata = pluginMetadata;
      nameIndex.plugin_metadata = pluginMetadataNames;
    }
    if (consumerGroups.length) {
      body.consumer_groups = consumerGroups;
      nameIndex.consumer_groups = consumerGroupNames;
    }

    return { body, nameIndex };
  }
}
