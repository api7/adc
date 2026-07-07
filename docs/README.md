# ADC Documentation

- [CLI Command Reference](./reference/cli.md) — every `adc` command, its flags, and sample usage.
- [Configuration Reference](./reference/configuration.md) — the `adc.yaml`/`adc.json` schema and variable syntax.
- [OpenAPI Converter Reference](./reference/openapi-converter.md) — the `x-adc-*` extension fields supported by `adc convert openapi`.
- [ADC Workflow Guide](./guides/workflow.md) — a walkthrough of the lint/diff/sync/dump loop, from configuring ADC to converting an OpenAPI spec.
- [Resource IDs](./guides/resource-ids.md) — how ADC assigns resource ids, and how to safely adopt resources that were previously managed through the Admin API or a UI.
- [Label Selector](./guides/label-selector.md) — how to let multiple teams or pipelines safely share a single backend with `--label-selector`.

For backend-specific details, see the backend READMEs:

- [Apache APISIX Backend](../libs/backend-apisix/README.md)
- [API7 Enterprise Backend](../libs/backend-api7/README.md)
- [OpenAPI Converter](../libs/converter-openapi/README.md)
