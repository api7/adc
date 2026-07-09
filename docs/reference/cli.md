# CLI Command Reference

API Declarative CLI (ADC) is a command-line tool for managing Apache APISIX and API7 Enterprise declaratively. ADC compares local configuration files with the backend state exposed by the Admin API and applies the changes needed to match the desired state.

The declarative configuration serves as the single source of truth that developers can manage through their existing version control systems.

ADC has the following general syntax:

```shell
adc [command] [options]
```

with global options:

- `-v, --version` to check the version
- `-h, --help` to print the help menu of the command

## Configuring ADC

Configure ADC with environment variables, a `.env` file, or command-line flags. Flags override environment variables for that command.

### Using Environment Variables

ADC exposes all configuration options as environment variables. For example, you can configure the backend type and address using the `ADC_BACKEND` and `ADC_SERVER` environment variables, respectively.

```shell
export ADC_BACKEND=api7ee
export ADC_SERVER=https://localhost:7443
```

You can also configure these options in a `.env` file, which ADC loads automatically:

```text title=".env"
ADC_BACKEND=api7ee
ADC_SERVER=https://localhost:7443
```

### Using Command-Line Flags

You can pass the same values as command-line flags. For example:

```shell
adc ping --backend api7ee --server "https://localhost:7443"
```

Run `adc help [command]` to see all options for a command.

## Common Backend Options

The options below are shared by every command that talks to a backend (`ping`, `sync`, `dump`, `diff`, `validate`):

| Option                                   | Default Value           | Description                                                                                                     | Valid Values                              | Env Variable               |
| ---------------------------------------- | ----------------------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | -------------------------- |
| `--verbose <integer>`                    | `1`                     | Verbosity level for logs. `0` no logs, `1` basic logs, `2` debug logs.                                          | `0`, `1` or `2`                           |                            |
| `--backend <backend>`                    | `apisix`                | Type of backend to connect to.                                                                                  | `apisix`, `api7ee` or `apisix-standalone` | `ADC_BACKEND`              |
| `--server <string>`                      | `http://localhost:9180` | HTTP address of the backend.                                                                                    |                                           | `ADC_SERVER`               |
| `--token <string>`                       |                         | Token for ADC to connect to the backend.                                                                        |                                           | `ADC_TOKEN`                |
| `--gateway-group <string>`               | `default`               | Gateway group to operate on (only supported for the `api7ee` backend).                                          |                                           | `ADC_GATEWAY_GROUP`        |
| `--label-selector <labelKey=labelValue>` |                         | Scope this command to resources matching the given label(s). See [Label Selector](../guides/label-selector.md). |                                           |                            |
| `--include-resource-type <string>`       |                         | Only include the specified resource type(s). Mutually exclusive with `--exclude-resource-type`.                 |                                           |                            |
| `--exclude-resource-type <string>`       |                         | Exclude the specified resource type(s). Mutually exclusive with `--include-resource-type`.                      |                                           |                            |
| `--timeout <duration>`                   | `10s`                   | Timeout for ADC to connect with the backend (examples: `30s`, `10m`, `1h10m`).                                  |                                           |                            |
| `--ca-cert-file <string>`                |                         | Path to the CA certificate to verify the backend.                                                               |                                           | `ADC_CA_CERT_FILE`         |
| `--tls-client-cert-file <string>`        |                         | Path to the mutual TLS client certificate to verify the backend.                                                |                                           | `ADC_TLS_CLIENT_CERT_FILE` |
| `--tls-client-key-file <string>`         |                         | Path to the mutual TLS client key to verify the backend.                                                        |                                           | `ADC_TLS_CLIENT_KEY_FILE`  |
| `--tls-skip-verify`                      | `false`                 | Disable verification of the backend TLS certificate.                                                            |                                           | `ADC_TLS_SKIP_VERIFY`      |

`--tls-client-cert-file` and `--tls-client-key-file` must be provided together.

## Commands

### `adc ping`

Check connectivity with the configured backend.

#### Sample Usage

```shell
# Check connectivity with the configured backend
adc ping

# Check connectivity with a specified backend
adc ping --backend apisix --server http://192.168.1.21:9180
```

### `adc lint`

Lint the local configuration file(s) to ensure it meets ADC requirements. Runs entirely locally; it does not connect to a backend.

| Option                   | Description      |
| ------------------------ | ---------------- |
| `-f, --file <file-path>` | File(s) to lint. |

#### Sample Usage

```shell
# Lint the specified configuration file
adc lint -f adc.yaml

# Lint multiple configuration files
adc lint -f service-a.yaml -f service-b.yaml
```

### `adc sync`

Synchronize the local configuration to the backend. This is a reconciliation operation: it creates, updates, and deletes resources on the backend to match the desired state described by the local file(s).

In addition to the [common backend options](#common-backend-options):

| Option                           | Default Value | Description                                                                  |
| -------------------------------- | ------------- | ---------------------------------------------------------------------------- |
| `-f, --file <file-path>`         |               | Configuration file(s) to synchronize. Repeatable; supports glob expressions. |
| `--no-lint`                      |               | Disable the lint check that runs before syncing.                             |
| `--request-concurrent <integer>` | `10`          | Number of concurrent requests sent to the backend.                           |

#### Sample Usage

```shell
# Synchronize configuration from a single file
adc sync -f adc.yaml

# Synchronize configuration from multiple files
adc sync -f service-a.yaml -f service-b.yaml

# Synchronize configuration in multiple files by glob expression
adc sync -f "**/*.yaml" -f common.yaml

# Synchronize configuration to a specific gateway group
adc sync -f adc.yaml --gateway-group production

# Synchronize configuration without lint check
adc sync -f adc.yaml --no-lint

# Synchronize configuration with debug logs
adc sync -f adc.yaml --verbose 2

# Synchronize configuration to a specific backend
adc sync -f service-a.yaml -f service-b.yaml --backend api7ee --server https://localhost:7443

# Synchronize only specified resource types from the configuration file
adc sync -f adc.yaml --include-resource-type global_rule --include-resource-type plugin_metadata

# Synchronize only the resources with the specified labels
adc sync -f adc.yaml --label-selector app=catalog
```

### `adc dump`

Save the configuration of the backend to a local file.

In addition to the [common backend options](#common-backend-options):

| Option                     | Default Value | Description                                                        |
| -------------------------- | ------------- | ------------------------------------------------------------------ |
| `-o, --output <file-path>` | `adc.yaml`    | Path of the file to save the configuration to.                     |
| `--with-id`                |               | Inline each resource's backend-assigned `id` into the dumped file. |

Use `--with-id` when adopting resources that were originally created outside ADC, such as resources created through a UI or the Admin API. It preserves the backend-assigned IDs in the dumped file so later syncs can update those resources in place instead of replacing them. See [Resource IDs](../guides/resource-ids.md).

#### Sample Usage

```shell
# Save backend configuration to the default adc.yaml file
adc dump

# Save backend configuration to a specified file
adc dump -o service-configuration.yaml

# Save only specified resource types from the backend
adc dump --include-resource-type global_rule --include-resource-type plugin_metadata

# Save only the resources with the specified labels
adc dump --label-selector app=catalog

# Save remote resource IDs
adc dump --with-id

# Save configuration from a specified backend
adc dump -o api7ee-dump.yaml --backend api7ee --server https://localhost:7443
```

### `adc diff`

Show the differences between the local configuration file(s) and the backend configuration. A detailed diff is also written to `diff.yaml` in the current directory.

In addition to the [common backend options](#common-backend-options):

| Option                   | Description                                                              |
| ------------------------ | ------------------------------------------------------------------------ |
| `-f, --file <file-path>` | Configuration file(s) to compare. Repeatable; supports glob expressions. |
| `--no-lint`              | Disable the lint check that runs before diffing.                         |

#### Sample Usage

```shell
# Compare configuration in a specified file with the backend configuration
adc diff -f adc.yaml

# Compare configuration in multiple specified files with the backend configuration
adc diff -f service-a.yaml -f service-b.yaml

# Compare configuration in multiple files by glob expression
adc diff -f "**/*.yaml" -f common.yaml

# Compare configuration in the specified files with a specific backend
adc diff -f service-a.yaml -f service-b.yaml --backend api7ee --server https://localhost:7443
```

### `adc validate`

Validate the local configuration file(s) against backend-specific validation rules without applying any changes. Unlike `lint`, which only checks local ADC schema rules, `validate` asks the configured backend to validate the resources described by the local file.

In addition to the [common backend options](#common-backend-options):

| Option                   | Description                                         |
| ------------------------ | --------------------------------------------------- |
| `-f, --file <file-path>` | File(s) to validate.                                |
| `--no-lint`              | Disable the lint check that runs before validating. |

#### Sample Usage

```shell
# Validate configuration from a single file
adc validate -f adc.yaml

# Validate configuration from multiple files
adc validate -f service-a.yaml -f service-b.yaml

# Validate configuration against the API7 Enterprise backend
adc validate -f adc.yaml --backend api7ee --gateway-group default

# Validate configuration without lint check
adc validate -f adc.yaml --no-lint
```

### `adc convert openapi`

Convert an OpenAPI specification to ADC configuration. The converter accepts OpenAPI 2.0 (Swagger) and OpenAPI 3.x specifications in either JSON or YAML format.

| Option                           | Default Value | Description                                           |
| -------------------------------- | ------------- | ----------------------------------------------------- |
| `-f, --file <openapi-file-path>` |               | OpenAPI specification file(s) to convert. Repeatable. |
| `-o, --output <output-path>`     | `adc.yaml`    | Output file path.                                     |

#### Sample Usage

```shell
# Convert OpenAPI specification in YAML format to ADC configuration and write to the default adc.yaml file
adc convert openapi -f openapi.yaml

# Convert OpenAPI specification in JSON format to ADC configuration and write to a specified file
adc convert openapi -f openapi.json -o converted-adc.yaml

# Convert multiple OpenAPI specifications to a single ADC configuration
adc convert openapi -f openapi.yaml -f openapi.json
```

To configure plugins, labels, upstream defaults, and other fields on the converted output, annotate the OpenAPI specification with `x-adc-*` extensions. See the [OpenAPI Converter Reference](./openapi-converter.md) for the full extension reference.

### `adc help`

Print the general help menu or the help menu for the specified command.

#### Sample Usage

```shell
adc help [command]
```
