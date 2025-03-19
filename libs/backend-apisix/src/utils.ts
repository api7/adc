import * as ADCSDK from '@api7/adc-sdk';

export const resourceTypeToAPIName = (resourceType: ADCSDK.ResourceType) => {
  switch (resourceType) {
    case ADCSDK.ResourceType.PLUGIN_METADATA:
      return resourceType;
    case ADCSDK.ResourceType.CONSUMER_CREDENTIAL:
      return `consumers/%s/credentials`;
    default:
      return `${resourceType}s`;
  }
};

export const capitalizeFirstLetter = (str: string) =>
  str.charAt(0).toUpperCase() + str.slice(1);
