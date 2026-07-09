# ADC Documentation

API Declarative CLI (ADC) manages Apache APISIX and API7 Enterprise configuration from local YAML or JSON files. Use it to export existing gateway configuration, review drift, validate proposed changes, and apply the desired state through the gateway Admin API.

Start with the workflow guide if you are setting up ADC for the first time. Use the reference pages when you need exact command flags, configuration fields, or OpenAPI conversion behavior.

## Get Started

- [Use ADC for Declarative Configuration](./guides/workflow.md): configure a backend, write an `adc.yaml` file, lint it, preview changes, sync it, and export backups.
- [Resource IDs](./guides/resource-ids.md): understand how ADC matches local resources to remote resources, especially before adopting resources that were created outside ADC.
- [Label Selector](./guides/label-selector.md): split ownership by labels so multiple teams or pipelines can manage one backend safely.

## Reference

- [CLI Command Reference](./reference/cli.md): commands, flags, backend options, and examples.
- [Configuration Reference](./reference/configuration.md): ADC configuration file structure, variables, resource fields, and links to the generated schema.
- [OpenAPI Converter Reference](./reference/openapi-converter.md): `adc convert openapi`, supported `x-adc-*` extensions, merge rules, and examples.

## Component Notes

These files describe implementation-specific behavior for ADC backends and converters:

- [Apache APISIX backend](../libs/backend-apisix/README.md)
- [API7 Enterprise backend](../libs/backend-api7/README.md)
- [OpenAPI converter](../libs/converter-openapi/README.md)
