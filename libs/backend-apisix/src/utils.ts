import * as ADCSDK from '@api7/adc-sdk';
import axios, { AxiosResponse } from 'axios';

export const resourceTypeToAPIName = (resourceType: ADCSDK.ResourceType) =>
  resourceType !== ADCSDK.ResourceType.PLUGIN_METADATA
    ? `${resourceType}s`
    : resourceType;

export const capitalizeFirstLetter = (str: string) =>
  str.charAt(0).toUpperCase() + str.slice(1);

export const buildReqAndRespDebugOutput = (
  resp: AxiosResponse,
  desc?: string,
) => {
  const config = resp.config;

  // NodeJS will not keep the response header in Xxx-Xxx format, correct it
  const normalizeHeaderKey = (key: string) =>
    key.split('-').map(capitalizeFirstLetter).join('-');

  // Transforms HTTP headers to a single line of text formatting
  const transformHeaders = (headers: object, normalizeKey = false) =>
    Object.entries(headers).map(
      ([key, value]) =>
        `${normalizeKey ? normalizeHeaderKey(key) : key}: ${key !== 'X-API-KEY' ? value : '*****'}\n`,
    );
  return JSON.stringify({
    type: 'debug',
    messages: [
      `${desc ?? ''}\n`, //TODO time consumption
      // request
      `${config.method.toUpperCase()} ${axios.getUri(config)}\n`,
      ...transformHeaders(config.headers),
      config?.data ? `\n${config.data}\n` : '',
      '\n',
      // response
      `${resp.status} ${resp.statusText}\n`,
      ...transformHeaders(resp.headers, true),
      `${resp?.data ? `\n${JSON.stringify(resp.data)}` : ''}\n`,
    ],
  });
};
