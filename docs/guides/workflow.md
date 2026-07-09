# Use ADC for Declarative Configuration

ADC lets you manage Apache APISIX and API7 Enterprise configuration from local files. A typical workflow is:

1. Connect ADC to a backend.
2. Export or write an `adc.yaml` file.
3. Lint and validate the file.
4. Preview the diff.
5. Sync the expected changes.
6. Keep the file in version control.

This gives teams a reviewable source of truth for gateway configuration and makes drift visible before changes reach production.

## Prerequisites

- ADC is installed. See [Installation](../../README.md#installation).
- An Apache APISIX or API7 Enterprise Admin API endpoint is reachable from the machine or CI runner that runs ADC.
- You have an Admin API key or API7 Enterprise dashboard token.
- For API7 Enterprise, you know the target gateway group.

## Configure the Backend

ADC reads configuration from command-line flags, environment variables, and `.env` files. Command-line flags take precedence over environment variables, and environment variables take precedence over `.env` values.

For API7 Enterprise:

```bash
export ADC_BACKEND=api7ee
export ADC_SERVER=https://localhost:7443
export ADC_TOKEN=<dashboard-token>
export ADC_GATEWAY_GROUP=default
```

For Apache APISIX:

```bash
export ADC_BACKEND=apisix
export ADC_SERVER=http://localhost:9180
export ADC_TOKEN=<admin-api-key>
```

You can store the same values in a `.env` file in the working directory:

```text title=".env"
ADC_BACKEND=api7ee
ADC_SERVER=https://localhost:7443
ADC_TOKEN=<dashboard-token>
ADC_GATEWAY_GROUP=default
```

For a one-off command, pass the options as flags:

```bash
adc ping --backend apisix --server http://localhost:9180 --token <admin-api-key>
```

If the backend uses a self-signed certificate, use `--ca-cert-file` with a trusted CA certificate. For local testing, you can use `--tls-skip-verify`.

Verify the connection:

```bash
adc ping
```

## Create a Configuration File

You can write a configuration file by hand, convert one from OpenAPI, or start from the current backend state.

To export the current backend configuration:

```bash
adc dump -o adc.yaml
```

If the backend already has resources that were created outside ADC, export the real backend IDs before adopting them:

```bash
adc dump --with-id -o adc.yaml
```

Keeping those IDs prevents ADC from replacing existing resources with newly generated IDs. See [Resource IDs](./resource-ids.md) before adopting UI-created or Admin API-created resources.

An ADC file can define services, routes, consumers, global rules, plugin metadata, and SSL certificates:

```yaml title="adc.yaml"
services:
  - name: httpbin-service
    upstream:
      name: default
      scheme: http
      type: roundrobin
      nodes:
        - host: httpbin.org
          port: 80
          weight: 100
    routes:
      - name: get-ip
        uris:
          - /ip
        methods:
          - GET
        plugins:
          key-auth: {}

consumers:
  - username: demo-user
    credentials:
      - name: primary-key
        type: key-auth
        config:
          key: <api-key>
```

See [Configuration Reference](../reference/configuration.md) for the supported fields.

## Check the File Locally

Run `adc lint` before comparing or syncing:

```bash
adc lint -f adc.yaml
```

`lint` checks local syntax and ADC schema rules. It does not connect to the backend.

## Validate Against the Backend

Run `adc validate` when you want backend-side validation without applying changes:

```bash
adc validate -f adc.yaml
```

`validate` asks the backend to validate the resources described by the local file. This is useful in CI because it catches issues that only the target backend can know, such as unsupported plugin configuration.

## Preview Changes

Run `adc diff` before applying a file:

```bash
adc diff -f adc.yaml
```

Review the output carefully. `adc diff` also writes a machine-readable `diff.yaml` file in the current directory.

## Apply Changes

After reviewing the diff, sync the file:

```bash
adc sync -f adc.yaml
```

`adc sync` reconciles the backend to match the local file. It can create, update, and delete resources that are in the command scope. A remote resource that is in scope but absent from the local file can be deleted.

The command scope is controlled by:

- `--gateway-group` for API7 Enterprise gateway groups.
- `--label-selector` for label-based ownership.
- `--include-resource-type` and `--exclude-resource-type` for resource type filters.

Use [Label Selector](./label-selector.md) if more than one team or pipeline manages the same backend.

## Convert OpenAPI to ADC

ADC can generate configuration from OpenAPI 2.0 and OpenAPI 3.x specifications:

```bash
adc convert openapi -f openapi.yaml -o adc.yaml
```

Plain OpenAPI documents describe APIs, not gateway-specific behavior. Add `x-adc-*` extensions when you need to control generated service names, route names, plugins, labels, upstream defaults, or route defaults. See [OpenAPI Converter Reference](../reference/openapi-converter.md).

## Suggested CI Flow

For a pull request or deployment pipeline, use this order:

```bash
adc lint -f adc.yaml
adc validate -f adc.yaml
adc diff -f adc.yaml
adc sync -f adc.yaml
```

Run `sync` only after the diff has been reviewed or approved by your release process.

## Related

- [CLI Command Reference](../reference/cli.md)
- [Configuration Reference](../reference/configuration.md)
- [Resource IDs](./resource-ids.md)
- [Label Selector](./label-selector.md)
