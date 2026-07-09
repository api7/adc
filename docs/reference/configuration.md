# Configuration Reference

ADC uses configuration files to define services, consumers, global rules, plugin metadata, and SSL certificates. The configuration file can be YAML or JSON and is the desired state that ADC commands read locally, compare, validate, and sync.

This document describes the configuration file structure and variable syntax. See the [CLI Command Reference](./cli.md) for CLI usage.

For the exhaustive schema, including every field and validation rule, use the generated `schema.json`. It is exported from the Zod schemas in `libs/sdk/src/core/schema.ts` with `nx run cli:export-schema`.

## Configuration File Structure

An ADC configuration file has the following top-level keys:

```yaml
services: # Service definitions with routes and upstreams
consumers: # Consumer definitions with credentials
global_rules: # Global plugin configurations
plugin_metadata: # Plugin metadata configurations
ssls: # SSL certificate configurations
```

All keys are optional, but `adc sync` is a reconciliation operation. A remote resource that is in the command scope but absent from the local file can be deleted. To manage only part of a backend, scope the command with `--include-resource-type`, `--exclude-resource-type`, or `--label-selector`.

## Example Configuration

```yaml title="adc.yaml"
services:
  - name: httpbin-service
    labels:
      deployment: production
    upstream:
      name: Default Upstream
      scheme: http
      type: roundrobin
      hash_on: vars
      nodes:
        - host: httpbin.org
          port: 80
          weight: 100
          priority: 0
      timeout:
        connect: 60
        send: 60
        read: 60
      retry_timeout: 0
      keepalive_pool:
        size: 320
        idle_timeout: 60
        requests: 1000
      pass_host: pass
    routes:
      - uris:
          - /ip
        name: get-ip
        labels:
          app: catalog
        methods:
          - GET
        enable_websocket: false
        priority: 0
      - uris:
          - /anything/*
        name: anything
        methods:
          - GET
          - POST
        plugins:
          proxy-rewrite:
            uri: /get

consumers:
  - username: internal-user
    labels:
      organisation: ACME
    credentials:
      - name: primary-key
        labels:
          type: internal
        type: key-auth
        config:
          key: <internal-api-key>

  - username: partner-user
    labels:
      organisation: API7.ai
    credentials:
      - name: primary-key
        type: key-auth
        config:
          key: <partner-api-key>

global_rules:
  prometheus:
    _meta:
      disable: false
    prefer_name: false
```

## Variables

ADC supports three variable syntaxes. Each one is resolved by a different component and at a different time.

The supported ADC versions vary by syntax:

| Syntax    | Resolved by | Minimum ADC version |
| --------- | ----------- | ------------------- |
| `$env://` | Data plane  | v0.12.1             |
| `${ENV}`  | ADC         | v0.12.0             |
| `${...}`  | Data plane  | v0.19.1             |

### Data Plane Environment Variables (`$env://`)

Use `$env://` to reference environment variables on the data plane instance at runtime. This is useful for values that differ across gateway instances or environments:

```yaml
services:
  - name: my-service
    upstream:
      nodes:
        - host: $env://UPSTREAM_HOST
          port: 80
```

The variable is resolved by the gateway when the configuration is loaded, not by ADC.

### ADC Environment Variables (`${ENV}`)

Use `${ENV}` for variables resolved by ADC while it loads the configuration file. This is useful for externalizing secrets or environment-specific values:

```yaml
consumers:
  - username: api-user
    credentials:
      - name: api-key
        type: key-auth
        config:
          key: ${API_KEY}
```

Set the environment variable before running ADC:

```bash
export API_KEY="<api-key>"
adc sync -f adc.yaml
```

Use `\${VAR_NAME}` to escape a literal `${VAR_NAME}` and prevent ADC from interpolating it.

### Built-in Variables (`${...}`)

Use `${...}` to reference built-in variables that the gateway evaluates per request.

Dotted gateway variables can be written directly because ADC does not treat them as ADC environment variables.

```yaml
plugins:
  limit-count:
    count: '${http_x_user_info.low_rate_limit}'
    key: '${http_x_user_info.rate_limit_key}'
    time_window: 60
    key_type: var
```

For literal variable names that match the ADC environment variable syntax, such as `${HOST}`, use `\${HOST}` to prevent ADC from interpolating them.

## Schema Reference

### Service

| Field               | Type    | Required | Description                                                                                      |
| ------------------- | ------- | -------- | ------------------------------------------------------------------------------------------------ |
| `id`                | string  | No       | Explicit backend ID. Keep this field when adopting resources exported with `adc dump --with-id`. |
| `name`              | string  | Yes      | Unique service name. If `id` is omitted, ADC generates the service ID from this name.            |
| `description`       | string  | No       | Free-form description.                                                                           |
| `labels`            | object  | No       | Key-value labels.                                                                                |
| `upstream`          | object  | No       | Upstream configuration.                                                                          |
| `routes`            | array   | No       | Route definitions.                                                                               |
| `stream_routes`     | array   | No       | Stream (TCP/UDP) route definitions.                                                              |
| `plugins`           | object  | No       | Service-level plugins.                                                                           |
| `path_prefix`       | string  | No       | Prefix prepended to every route URI in this service.                                             |
| `strip_path_prefix` | boolean | No       | Strip the matched route prefix before forwarding.                                                |

### Route (nested under service)

| Field              | Type    | Required | Description                             |
| ------------------ | ------- | -------- | --------------------------------------- |
| `id`               | string  | No       | Explicit backend ID.                    |
| `name`             | string  | Yes      | Route name (unique within the service). |
| `uris`             | array   | Yes      | URI paths to match.                     |
| `methods`          | array   | No       | HTTP methods to match.                  |
| `hosts`            | array   | No       | Host headers to match.                  |
| `labels`           | object  | No       | Key-value labels.                       |
| `plugins`          | object  | No       | Route-level plugins.                    |
| `priority`         | integer | No       | Route matching priority (higher wins).  |
| `enable_websocket` | boolean | No       | Enable WebSocket proxying.              |

### Upstream (nested under service)

| Field            | Type   | Required | Description                                                                                      |
| ---------------- | ------ | -------- | ------------------------------------------------------------------------------------------------ |
| `id`             | string | No       | Explicit backend ID for named upstreams.                                                         |
| `name`           | string | No       | Upstream name.                                                                                   |
| `scheme`         | string | No       | Protocol: `http`, `https`, `grpc`, `grpcs`.                                                      |
| `type`           | string | No       | Load balancing algorithm: `roundrobin`, `chash`, `ewma`, `least_conn`.                           |
| `hash_on`        | string | No       | Hash key source for `chash`: `vars`, `header`, `cookie`, `consumer`.                             |
| `nodes`          | array  | No       | Backend nodes (`host`, `port`, `weight`, `priority`). Required unless service discovery is used. |
| `service_name`   | string | No       | Service discovery name. Use with `discovery_type` instead of `nodes`.                            |
| `discovery_type` | string | No       | Service discovery type. Use with `service_name` instead of `nodes`.                              |
| `discovery_args` | object | No       | Additional service discovery arguments.                                                          |
| `timeout`        | object | No       | Timeout settings (`connect`, `send`, `read`) in seconds.                                         |
| `pass_host`      | string | No       | Host header behavior: `pass`, `node`, `rewrite`.                                                 |
| `keepalive_pool` | object | No       | Connection pool settings.                                                                        |
| `retry_timeout`  | number | No       | Retry timeout in seconds (`0` = no limit).                                                       |
| `checks`         | object | No       | Active/passive health check configuration.                                                       |

### Consumer

| Field         | Type   | Required | Description                 |
| ------------- | ------ | -------- | --------------------------- |
| `username`    | string | Yes      | Unique consumer name.       |
| `labels`      | object | No       | Key-value labels.           |
| `credentials` | array  | No       | Authentication credentials. |
| `plugins`     | object | No       | Consumer-level plugins.     |

### Credential (nested under consumer)

| Field    | Type   | Required | Description                                                             |
| -------- | ------ | -------- | ----------------------------------------------------------------------- |
| `id`     | string | No       | Explicit backend ID.                                                    |
| `name`   | string | Yes      | Credential name.                                                        |
| `type`   | string | Yes      | Authentication type: `key-auth`, `basic-auth`, `jwt-auth`, `hmac-auth`. |
| `config` | object | Yes      | Type-specific configuration.                                            |
| `labels` | object | No       | Key-value labels.                                                       |

### Global Rules

Global rules are defined as a map of plugin name to plugin configuration, applied to every request regardless of route:

```yaml
global_rules:
  plugin-name:
    _meta:
      disable: false
    # plugin-specific configuration
```

### Plugin Metadata

Plugin metadata configures shared, plugin-wide settings (as opposed to per-route/per-service plugin configuration), defined as a map of plugin name to metadata:

```yaml
plugin_metadata:
  plugin-name:
    # plugin-specific metadata
```

## Additional Resources

- [CLI Command Reference](./cli.md)
- [ADC Workflow Guide](../guides/workflow.md)
