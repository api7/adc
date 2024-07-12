# API Declarative CLI (ADC)

ADC is a command line utility to interface with API7 Enterprise and Apache APISIX Admin API.

## Supported Backend

| Backend         | Documentation                           |
| --------------- | --------------------------------------- |
| API7 Enterprise | [README](libs/backend-api7/README.md)   |
| Apache APISIX   | [README](libs/backend-apisix/README.md) |

## Supported Converter

| Converter      | Documentation                              |
| -------------- | ------------------------------------------ |
| OpenAPI Spec 3 | [README](libs/converter-openapi/README.md) |

## Installation

You can download the appropriate binary from the [releases page](https://github.com/api7/adc/releases):

```bash
wget https://github.com/api7/adc/releases/download/v0.10.0/adc_0.10.0_linux_amd64.tar.gz
tar -zxvf adc_0.10.0_linux_amd64.tar.gz
mv adc /usr/local/bin/adc
```

Pre-built binaries for `amd64` and `arm64` on Linux, Windows, and macOS are available now.

## Usage

### Initial setup

The easiest way to persist configurations is to use environment variables. You can check the environment variables in `adc help`.

The ADC supports dotenv, so you can create `.env` files to store environment variables.

You need to set the `token` to the ADC so that the ADC can access API7 Enterprise and the Apache APISIX Admin API.

#### API7 Enterprise

```bash
ADC_BACKEND=api7ee
ADC_SERVER=https://localhost:7443
ADC_TOKEN=<generate token on API7 Dashboard>
```

#### Apache APISIX

```bash
ADC_SERVER=http://localhost:9180
ADC_TOKEN=<Admin API key in the configuration file>
```

### Common commands

#### Check connectivity

```bash
adc ping
```

#### Dump configurations to ADC format

```bash
adc dump -o adc.yaml
```

#### Check for differences between local and remote configurations

```bash
adc diff -f adc.yaml
```

#### Synchronize local configuration

```bash
adc sync -f adc.yaml
```

#### Convert API configuration

For example, it supports the conversion of OpenAPI 3 to ADC configurations.

```bash
adc convert openapi -f openapi.yaml
```

#### Verify ADC configuration

```bash
adc lint -f adc.yaml
```

## License

This project is licensed under the [Apache 2.0 License](LICENSE).
