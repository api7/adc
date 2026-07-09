# API Declarative CLI (ADC)

API Declarative CLI (ADC) is a command-line tool for managing Apache APISIX and API7 Enterprise configuration declaratively. ADC reads local YAML or JSON files, compares them with the gateway configuration exposed by the Admin API, and applies the changes needed to make the gateway match the desired state.

Use ADC when you want gateway configuration to be reviewed, versioned, promoted, and restored like application code.

## Features

- Export gateway configuration to an ADC file with `adc dump`.
- Preview configuration drift with `adc diff`.
- Apply local configuration with `adc sync`.
- Validate local files before applying changes with `adc lint` and `adc validate`.
- Convert OpenAPI specifications to ADC configuration with `adc convert openapi`.
- Scope ownership by resource type or label so different teams can manage separate parts of a shared backend.

## Supported Backends

ADC supports the following backend types:

- [API7 Enterprise](libs/backend-api7/README.md)
- [Apache APISIX](libs/backend-apisix/README.md)
- Apache APISIX standalone mode

## Installation

Install ADC with the install script:

```bash
curl -sL "https://run.api7.ai/adc/install" | sh
```

You can also download a pre-built binary for Linux, macOS, or Windows from the [releases page](https://github.com/api7/adc/releases).

Verify the installation:

```bash
adc --help
```

## Quick Start

Configure the backend with environment variables or a `.env` file.

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

Check connectivity:

```bash
adc ping
```

Export the current configuration:

```bash
adc dump -o adc.yaml
```

Preview local changes before applying them:

```bash
adc diff -f adc.yaml
```

Apply the local configuration:

```bash
adc sync -f adc.yaml
```

## Documentation

See [docs/](docs/README.md) for the workflow guide and reference documentation:

- [Use ADC for Declarative Configuration](docs/guides/workflow.md)
- [CLI Command Reference](docs/reference/cli.md)
- [Configuration Reference](docs/reference/configuration.md)
- [OpenAPI Converter Reference](docs/reference/openapi-converter.md)
- [Resource IDs](docs/guides/resource-ids.md)
- [Label Selector](docs/guides/label-selector.md)

## Development

To build ADC from source, [install Nx](https://nx.dev/getting-started/installation) and run:

```bash
pnpm install
nx build cli
```

To use the binary, run:

```bash
node dist/apps/cli/main.cjs -h
```

## License

This project is licensed under the [Apache 2.0 License](LICENSE).
