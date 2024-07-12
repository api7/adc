# API Declarative CLI (ADC)

ADC is a command line utility that interfaces with API7 Enterprise and Apache APISIX Admin APIs.

## Supported Backends

The following backend types are supported in ADC:

1. [API7 Enterprise](libs/backend-api7/README.md)
2. [Apache APISIX](libs/backend-apisix/README.md)

## Supported Converters

The following converters are supported to convert API specifications to ADC configuration:

1. [OpenAPI Spec 3](libs/converter-openapi/README.md)

## Installation

You can download the appropriate binary from the [releases page](https://github.com/api7/adc/releases):

```bash
wget https://github.com/api7/adc/releases/download/v0.10.0/adc_0.10.0_linux_amd64.tar.gz
tar -zxvf adc_0.10.0_linux_amd64.tar.gz
mv adc /usr/local/bin/adc
```

Pre-built binaries for `amd64` and `arm64` on Linux, Windows, and macOS are available now.

## Configure ADC

You can configure ADC through environment variables or command line flags. Run `adc help [command]` to see the available configuration options for a command.

ADC supports dotenv, so you can store and use your environment variables in a `.env` file. The examples below show how to configure ADC for both API7 Enterprise and Apache APISIX backends.

### Example API7 Enterprise Configuration

```bash
ADC_BACKEND=api7ee
ADC_SERVER=https://localhost:7443
ADC_TOKEN=<token generated from the dashboard>
```

### Example Apache APISIX Configuration

```bash
ADC_SERVER=http://localhost:9180
ADC_TOKEN=<APISIX Admin API key>
```

## Usage

This section highlights some of the common ADC commands.

### Check Connectivity

The `ping` command verifies the configuration by trying to connect to the configured backend:

```bash
adc ping
```

### Dump Configuration in ADC Format

The `dump` command fetches the current configuration of the backend and saves it in the ADC configuration file format:

```bash
adc dump -o adc.yaml
```

### Show the Difference between Local and Remote Configuration

The `diff` command compares the configuration in the specified ADC configuration file with the current configuration of the backend:

```bash
adc diff -f adc.yaml
```

### Synchronize Local Configuration

The `sync` command synchronizes the configuration in the specified ADC configuration file with the backend:

```bash
adc sync -f adc.yaml
```

### Convert API Specifications

The `convert` command converts API specifications to ADC configuration. Currently, it supports converting an OpenAPI 3 specification to ADC configuration.

```bash
adc convert openapi -f openapi.yaml
```

### Verify ADC Configuration

The `lint` command verifies the provided ADC configuration file locally.

```bash
adc lint -f adc.yaml
```

## Development

To build ADC from source, [install Nx](https://nx.dev/getting-started/installation) and run:

```bash
pnpm install
nx build cli
```

To use the binary, run:

```bash
node dist/apps/cli/main.js -h
```

## License

This project is licensed under the [Apache 2.0 License](LICENSE).
