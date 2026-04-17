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
    config: ADCSDK.Configuration,
  ): Promise<ADCSDK.BackendValidateResult> {
    const { body, nameIndex } = this.buildRequestBody(config);

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

  private buildRequestBody(config: ADCSDK.Configuration): {
    body: ValidateRequestBody;
    nameIndex: Record<string, string[]>;
  } {
    const body: ValidateRequestBody = {};
    const nameIndex: Record<string, string[]> = {};

    if (config.services?.length) {
      const services: Array<typing.Service> = [];
      const routes: Array<typing.Route> = [];
      const streamRoutes: Array<typing.StreamRoute> = [];
      const serviceNames: string[] = [];
      const routeNames: string[] = [];
      const streamRouteNames: string[] = [];

      for (const service of config.services) {
        const serviceId =
          service.id ?? ADCSDK.utils.generateId(service.name);
        const svc = { ...service, id: serviceId };
        const transformed = this.fromADC.transformService(svc);
        services.push(transformed);
        serviceNames.push(service.name);

        for (const route of service.routes ?? []) {
          const routeId = route.id ?? ADCSDK.utils.generateId(route.name);
          const r = { ...route, id: routeId };
          routes.push(this.fromADC.transformRoute(r, serviceId));
          routeNames.push(route.name);
        }

        for (const streamRoute of service.stream_routes ?? []) {
          const streamRouteId =
            streamRoute.id ?? ADCSDK.utils.generateId(streamRoute.name);
          const sr = { ...streamRoute, id: streamRouteId };
          streamRoutes.push(
            this.fromADC.transformStreamRoute(sr, serviceId),
          );
          streamRouteNames.push(streamRoute.name);
        }
      }

      body.services = services;
      nameIndex.services = serviceNames;
      if (routes.length) {
        body.routes = routes;
        nameIndex.routes = routeNames;
      }
      if (streamRoutes.length) {
        body.stream_routes = streamRoutes;
        nameIndex.stream_routes = streamRouteNames;
      }
    }

    if (config.consumers?.length) {
      body.consumers = config.consumers.map((c) =>
        this.fromADC.transformConsumer(c),
      );
      nameIndex.consumers = config.consumers.map((c) => c.username);
    }

    if (config.ssls?.length) {
      body.ssls = config.ssls.map((ssl) => {
        const sslId = ssl.id ?? ADCSDK.utils.generateId(ssl.snis?.[0] ?? '');
        return this.fromADC.transformSSL({ ...ssl, id: sslId });
      });
      nameIndex.ssls = config.ssls.map((ssl) => ssl.snis?.[0] ?? '');
    }

    if (config.global_rules && Object.keys(config.global_rules).length) {
      body.global_rules = this.fromADC.transformGlobalRule(
        config.global_rules as Record<string, ADCSDK.GlobalRule>,
      );
      nameIndex.global_rules = Object.keys(config.global_rules);
    }

    if (
      config.plugin_metadata &&
      Object.keys(config.plugin_metadata).length
    ) {
      body.plugin_metadata = Object.entries(config.plugin_metadata).map(
        ([pluginName, config]) => ({
          id: pluginName,
          ...ADCSDK.utils.recursiveOmitUndefined(config),
        }),
      );
      nameIndex.plugin_metadata = Object.keys(config.plugin_metadata);
    }

    if (config.consumer_groups?.length) {
      body.consumer_groups = config.consumer_groups.map((cg) => {
        const id = ADCSDK.utils.generateId(cg.name);
        return ADCSDK.utils.recursiveOmitUndefined({
          id,
          name: cg.name,
          desc: cg.description,
          labels: cg.labels,
          plugins: cg.plugins,
        }) as unknown as Record<string, unknown>;
      });
      nameIndex.consumer_groups = config.consumer_groups.map((cg) => cg.name);
    }

    return { body, nameIndex };
  }
}
