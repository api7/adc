# Use ADC for Declarative Configuration

ADC (API Declarative CLI) lets you manage your gateway configuration declaratively using YAML files. This enables GitOps workflows, version-controlled configuration, and reproducible deployments across different environments.

Instead of making imperative API calls, you define the desired state in a YAML file and ADC reconciles it with the gateway.

## Prerequisites

- An Apache APISIX or API7 Enterprise instance is running and its Admin API is reachable.
- ADC is installed. See the [installation instructions](../../README.md#installation).
- For API7 Enterprise: a Gateway Group is created, a Gateway instance is running, and you have a token to access the Admin API.

## Configure ADC

ADC can be configured through environment variables, a `.env` file, or command-line flags. All three sources expose the same options; flags take precedence over environment variables, which take precedence over `.env` values.

### Using Environment Variables

For an API7 Enterprise backend:

```bash
export ADC_BACKEND=api7ee
export ADC_SERVER=https://localhost:7443
export ADC_TOKEN=${API_KEY}
export ADC_GATEWAY_GROUP=default
```

For an Apache APISIX backend:

```bash
export ADC_BACKEND=apisix
export ADC_SERVER=http://localhost:9180
export ADC_TOKEN=${ADMIN_API_KEY}
```

### Using a `.env` File

ADC loads a `.env` file in the working directory automatically (via `dotenv`), so you can persist the same variables instead of exporting them in your shell:

```text title=".env"
ADC_BACKEND=api7ee
ADC_SERVER=https://localhost:7443
ADC_TOKEN=${API_KEY}
ADC_GATEWAY_GROUP=default
```

### Using Command-Line Flags

Every option is also available as a flag, which overrides whatever is set via environment variables or `.env` for that single invocation — useful for one-off commands or targeting a different backend without changing your shell state:

```bash
adc ping --backend apisix --server http://localhost:9180 --token ${ADMIN_API_KEY}
```

If you are connecting to a local instance with a self-signed certificate, add `--tls-skip-verify` to the ADC command (or set `ADC_TLS_SKIP_VERIFY=true`).

See the [CLI Command Reference](../reference/cli.md#common-backend-options) for the full list of configuration options and their corresponding environment variables.

Verify connectivity:

```bash
adc ping
```

## Write a Declarative Configuration

Define your gateway resources such as services, routes, and consumers in an `adc.yaml` file.

```yaml title="adc.yaml"
services:
  - name: adc-workflow-httpbin-service
    upstream:
      name: default
      scheme: http
      type: roundrobin
      nodes:
        - host: httpbin.org
          port: 80
          weight: 100
    routes:
      - name: adc-workflow-route
        uris:
          - /anything/adc-workflow
        methods:
          - GET
        plugins:
          key-auth:
            header: apikey

consumers:
  - username: adc-workflow-consumer
    credentials:
      - name: adc-workflow-key
        type: key-auth
        config:
          key: adc-workflow-api-key
```

The configuration file uses top-level keys to organize resources such as `services`, `consumers`, `global_rules`, and `plugin_metadata`. See the [Configuration Reference](../reference/configuration.md) for the full schema.

## Validate the Configuration Locally

Before applying changes, use the `lint` command to check your configuration for syntax errors and schema violations.

```bash
adc lint -f adc.yaml
```

## Preview Changes

The `diff` command shows you exactly what changes ADC will make to the gateway configuration to match your local file.

```bash
adc diff -f adc.yaml
```

This output helps you avoid accidental deletions or modifications before applying the configuration. A detailed, machine-readable diff is also written to `diff.yaml`.

## Apply the Configuration

Once you have verified the changes, use the `sync` command to apply the configuration to the gateway.

```bash
adc sync -f adc.yaml
```

> **Caution**: `adc sync` is a reconciliation operation. It will create, update, and delete resources to match the desired state in your YAML file. Resources that exist on the gateway but are not in the YAML file will be removed.

If you only want to check whether a sync would succeed — for example in a CI pipeline — without applying it, use `adc validate` instead, which computes the diff and runs backend-side validation without writing any changes.

## Back Up the Current Configuration

You can export the current gateway configuration to a YAML file using the `dump` command. This is useful for creating backups or migrating configurations between environments.

```bash
adc dump -o backup.yaml
```

## Convert from OpenAPI

ADC can generate a declarative configuration from an existing OpenAPI specification file. Both OpenAPI 2.0 (Swagger) and OpenAPI 3.x are accepted, in either JSON or YAML format.

```bash
adc convert openapi -f openapi.yaml -o adc.yaml
```

A plain OpenAPI document only describes endpoints. To configure gateway-specific behavior on the converted output — plugins, labels, upstream defaults, route defaults, and so on — annotate the specification with `x-adc-*` extension fields. See the [OpenAPI Converter Reference](../reference/openapi-converter.md) for the full extension reference, processing rules, and examples.

## A Typical ADC Workflow

1. **Lint**: `adc lint -f adc.yaml`
2. **Diff**: `adc diff -f adc.yaml`
3. **Sync**: `adc sync -f adc.yaml`
4. **Dump**: `adc dump -o backup.yaml`

## Next Steps

- [CLI Command Reference](../reference/cli.md) — see all available commands and options.
- [Configuration Reference](../reference/configuration.md) — full configuration file schema and variable syntax.
- [Resource IDs](./resource-ids.md) — required reading before pointing ADC at a backend that already has resources created through the Admin API or a UI.
