# ADC

The ADC CLI tool is a command-line interface for interacting with the APISIX's API. It is built using the Golang Cobra library and provides several sub-commands for managing your APISIX instance.

## Installation

The ADC CLI tool can be installed using the `go install` command:

```
go install github.com/api7/adc@latest
```

This will install the `adc` binary to your `$GOPATH/bin` directory. Make sure that this directory is in your `$PATH` environment variable.

You can also download the binary from the [releases page](https://github.com/api7/adc/releases) and place it in your `$PATH` directory.

```bash
# Download the binary
wget https://github.com/api7/adc/releases/download/v0.1.0/adc_0.1.0_linux_amd64.tar.gz
tar -zxvf adc_0.1.0_linux_amd64.tar.gz
mv adc /usr/local/bin/adc
```

## Usage

To use the ADC CLI tool, run the `adc` command followed by the sub-command you wish to use. For example, to check the connection to APISIX API, run the following command:

```
adc ping
```

This will verify the connection to the APISIX API and print a success message.

The following sub-commands are available:

- `configure`: Configure the ADC CLI tool with the APISIX's server address and token.
- `ping`: Verify the connection to the APISIX API.
- `sync`: Sync your local configuration to APISIX instance.
- `diff`: Show the differences between your local configuration and the APISIX instance.
- `dump`: Dump your APISIX configurations to a local file.

Use the `--help` flag with any sub-command to see its usage information.

## Configuration

The ADC CLI tool can be configured using the `configure` sub-command. This sub-command will prompt you for your APISIX server address and API token, and save them to a configuration file.

The configuration file is located at `$HOME/.adc.yaml` and can be edited manually if necessary.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
