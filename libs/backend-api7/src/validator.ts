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
    const body = this.buildRequestBody(config);

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
        return {
          success: false,
          errorMessage: data?.error_msg,
          errors: data?.errors ?? [],
        };
      }
      throw error;
    }
  }

  private buildRequestBody(config: ADCSDK.Configuration): ValidateRequestBody {
    const body: ValidateRequestBody = {};

    if (config.services?.length) {
      const services: Array<typing.Service> = [];
      const routes: Array<typing.Route> = [];
      const streamRoutes: Array<typing.StreamRoute> = [];

      for (const service of config.services) {
        const serviceId =
          service.id ?? ADCSDK.utils.generateId(service.name);
        const svc = { ...service, id: serviceId };
        const transformed = this.fromADC.transformService(svc);
        services.push(transformed);

        for (const route of service.routes ?? []) {
          const routeId = route.id ?? ADCSDK.utils.generateId(route.name);
          const r = { ...route, id: routeId };
          routes.push(this.fromADC.transformRoute(r, serviceId));
        }

        for (const streamRoute of service.stream_routes ?? []) {
          const streamRouteId =
            streamRoute.id ?? ADCSDK.utils.generateId(streamRoute.name);
          const sr = { ...streamRoute, id: streamRouteId };
          streamRoutes.push(
            this.fromADC.transformStreamRoute(sr, serviceId),
          );
        }
      }

      body.services = services;
      if (routes.length) body.routes = routes;
      if (streamRoutes.length) body.stream_routes = streamRoutes;
    }

    if (config.consumers?.length) {
      body.consumers = config.consumers.map((c) =>
        this.fromADC.transformConsumer(c),
      );
    }

    if (config.ssls?.length) {
      body.ssls = config.ssls.map((ssl) => {
        const sslId = ssl.id ?? ADCSDK.utils.generateId(ssl.snis?.[0] ?? '');
        return this.fromADC.transformSSL({ ...ssl, id: sslId });
      });
    }

    if (config.global_rules && Object.keys(config.global_rules).length) {
      body.global_rules = this.fromADC.transformGlobalRule(
        config.global_rules as Record<string, ADCSDK.GlobalRule>,
      );
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
    }

    return body;
  }
}
