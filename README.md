# ADC

The ADC CLI tool is a command-line interface for interacting with the API7 API. It is built using the Golang Cobra library and provides several sub-commands for managing your API7 account.

## Usage

To use the ADC CLI tool, run the `adc` command followed by the sub-command you wish to use. For example, to ping the API7 API, run the following command:

```
adc ping
```

This will verify the connection to the ADC API and print a success message.

The following sub-commands are available:

- `ping`: Verify the connection to the ADC API.
- `sync`: Sync your local data with the ADC API.
- `diff`: Show the differences between your local data and the ADC API.
- `dump`: Dump your local data to a file.
- `configure`: Configure the ADC CLI tool.

Use the `--help` flag with any sub-command to see its usage information.

## Configuration

The ADC CLI tool can be configured using the `configure` sub-command. This sub-command will prompt you for your ADC server address and API token, and save them to a configuration file.

The configuration file is located at `$HOME/.adc.yaml` and can be edited manually if necessary.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.