# APISIX Declarative CLI (ADC)

ADC is a command line utility to interface with APISIX's API.

It is built using the [Cobra](https://github.com/spf13/cobra) library.

## Installation

### macOS/Linux

You can install ADC using the install script by running:

```shell
curl -sL https://run.api7.ai/adc/install | sh
```

To install a specific version or set a custom install directory, use the `ADC_VERSION` and `ADC_DIR` environment variables respectievely:

```shell
export ADC_VERSION=0.5.0
export ADC_DIR=/bin
```

You can also download the appropriate binary from the [releases page](https://github.com/api7/adc/releases):

```bash
wget https://github.com/api7/adc/releases/download/v0.5.0/adc_0.5.0_linux_amd64.tar.gz
tar -zxvf adc_0.5.0_linux_amd64.tar.gz
mv adc /usr/local/bin/adc
```

> [!IMPORTANT]
> Make sure that these directories are in your `$PATH` environment variable.

### Windows

You can download the release for Windows from the [releases page](https://github.com/api7/adc/releases) and extract it. You can then use the `adc.exe` executable to run ADC.

## Usage

To view a list of all available commands, run:

```shell
adc --help
```

To learn how to use a particular subcommand, for example, `ping` run:

```shell
adc ping --help
```

### adc configure

```shell
adc configure
```

Configures ADC with APISIX's server address and token. Running this command will prompt you for an APISIX server address and API token and saves them to a configuration file.

By default, ADC creates a configuration file at `$HOME/apisix.yaml` and this can be changed manually.

### adc ping

```shell
adc ping
```

Pings the configured APISIX instance to verify connectivity.

### adc validate

```shell
adc validate -f apisix.yaml
```

Validates the provided APISIX configuration file.

### adc sync

```shell
adc sync
```

Syncs the local configuration present in the `$HOME/apisix.yaml` file (or specified configuration file) to the connected APISIX instance.

### adc dump

```shell
adc dump --output apisix.yaml
```

Dumps the configuration of the connected APISIX instance to the specified configuration file.

### adc diff

```shell
adc diff
```

Shows the differences in configuration between the connected APISIX instance and the local configuration file.

### adc openapi2apisix

```shell
adc openapi2apisix -o apisix.yaml -f openAPI.yaml
```

Converts the configuration in OpenAPI format (`openAPI.yaml`) to APISIX configuration (`apisix.yaml`).

### adc version

```shell
adc version
```

Prints the version of ADC. See [Installation](#installation) for details on installing the latest version.

### adc completion

```shell
adc completion <bash|zsh|fish|powershell>
```

Generates autocompletion scripts for the specified shell.

## License

This project is licensed under the [Apache 2.0 License](LICENSE).
