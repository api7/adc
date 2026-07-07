# Configuration Reference

ADC uses configuration files to define services, consumers, global rules, plugin metadata, and SSL certificates. The configuration file can be in YAML or JSON format and serves as the single source of truth for declarative management.

This document describes the configuration file structure and variable syntax. See the [CLI Command Reference](./cli.md) for CLI usage.

> For the exhaustive, always up-to-date schema (including every field and validation rule), see `schema.json`, generated from the Zod schemas in `libs/sdk/src/core/schema.ts` via `nx run cli:export-schema`.

## Configuration File Structure

An ADC configuration file has the following top-level keys:

```yaml
services:        # Service definitions with routes and upstreams
consumers:        # Consumer definitions with credentials
global_rules:     # Global plugin configurations
plugin_metadata:  # Plugin metadata configurations
ssls:             # SSL certificate configurations
```

All keys are optional. You can define a partial configuration that only includes the resources you want to manage; resources of a type that is omitted entirely are left untouched on the backend. To manage only specific resource types during `sync`/`dump`/`diff`, use `--include-resource-type`/`--exclude-resource-type` (see the [CLI Command Reference](./cli.md)).

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
  - username: jane
    labels:
      organisation: ACME
    credentials:
      - name: primary-key
        labels:
          type: internal
        type: key-auth
        config:
          key: c1_yN0nCWousUfiR4EzfH

  - username: john
    labels:
      organisation: API7.ai
    credentials:
      - name: primary-key
        type: key-auth
        config:
          key: EIul6mAuYkLJ27on1aJD4

global_rules:
  prometheus:
    _meta:
      disable: false
    prefer_name: false
```

## Variables

ADC supports three variable syntaxes, each resolving values from different sources.

The supported ADC versions vary by syntax:

| Syntax     | Resolved by | Minimum ADC version |
| ---------- | ----------- | -------------------- |
| `$env://`  | Data plane  | v0.12.1               |
| `${ENV}`   | ADC         | v0.12.0               |
| `\${}`     | Data plane  | v0.19.1               |

### Data Plane Environment Variables (`$env://`)

Use the `$env://` syntax to reference environment variables on the data plane (gateway) instance at runtime. This is useful for values that differ across gateway instances or environments:

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

Use the `${ENV}` syntax for variables resolved by ADC itself when processing the configuration file. This is useful for externalizing secrets or environment-specific values:

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
export API_KEY="my-secret-key"
adc sync -f adc.yaml
```

Use `\${VAR_NAME}` to escape a literal `${VAR_NAME}` and prevent ADC from interpolating it.

### Built-in Variables (`\${}`)

Use the `\${}` syntax (escaped dollar sign) to reference built-in variables that are evaluated per-request by the gateway:

In YAML double-quoted strings, write the escape sequence as `\\${...}` so that ADC receives `\${...}` after YAML parsing. In single-quoted strings or unquoted values, you can write `\${...}` directly.

```yaml
plugins:
  limit-count:
    count: "\\${http_x_user_info.low_rate_limit}"
    key: "\\${http_x_user_info.rate_limit_key}"
    time_window: 60
    key_type: var
```

## Schema Reference

### Service

| Field                | Type    | Required | Description                                        |
| --------------------- | ------- | -------- | --------------------------------------------------- |
| `name`               | string  | Yes      | Unique service name. This is not the service ID, which is generated by the backend. |
| `description`        | string  | No       | Free-form description.                             |
| `labels`             | object  | No       | Key-value labels.                                  |
| `upstream`           | object  | No       | Upstream configuration.                            |
| `routes`             | array   | No       | Route definitions.                                 |
| `stream_routes`      | array   | No       | Stream (TCP/UDP) route definitions.                |
| `plugins`            | object  | No       | Service-level plugins.                              |
| `path_prefix`        | string  | No       | Prefix prepended to every route URI in this service. |
| `strip_path_prefix`  | boolean | No       | Strip the matched route prefix before forwarding.   |

### Route (nested under service)

| Field              | Type    | Required | Description                                  |
| ------------------- | ------- | -------- | ---------------------------------------------- |
| `name`             | string  | Yes      | Route name (unique within the service).       |
| `uris`             | array   | Yes      | URI paths to match.                            |
| `methods`          | array   | No       | HTTP methods to match.                         |
| `hosts`            | array   | No       | Host headers to match.                         |
| `labels`           | object  | No       | Key-value labels.                              |
| `plugins`          | object  | No       | Route-level plugins.                           |
| `priority`         | integer | No       | Route matching priority (higher wins).         |
| `enable_websocket` | boolean | No       | Enable WebSocket proxying.                     |

### Upstream (nested under service)

| Field             | Type   | Required | Description                                                             |
| ------------------ | ------ | -------- | -------------------------------------------------------------------------- |
| `name`            | string | No       | Upstream name.                                                          |
| `scheme`          | string | No       | Protocol: `http`, `https`, `grpc`, `grpcs`.                             |
| `type`            | string | No       | Load balancing algorithm: `roundrobin`, `chash`, `ewma`, `least_conn`.  |
| `hash_on`         | string | No       | Hash key source for `chash`: `vars`, `header`, `cookie`, `consumer`.    |
| `nodes`           | array  | Yes      | Backend nodes (`host`, `port`, `weight`, `priority`).                   |
| `timeout`         | object | No       | Timeout settings (`connect`, `send`, `read`) in seconds.                |
| `pass_host`       | string | No       | Host header behavior: `pass`, `node`, `rewrite`.                        |
| `keepalive_pool`  | object | No       | Connection pool settings.                                               |
| `retry_timeout`   | number | No       | Retry timeout in seconds (`0` = no limit).                              |
| `checks`          | object | No       | Active/passive health check configuration.                              |

### Consumer

| Field         | Type   | Required | Description                     |
| -------------- | ------ | -------- | ---------------------------------- |
| `username`    | string | Yes      | Unique consumer name.            |
| `labels`      | object | No       | Key-value labels.                 |
| `credentials` | array  | No       | Authentication credentials.       |
| `plugins`     | object | No       | Consumer-level plugins.           |

### Credential (nested under consumer)

| Field    | Type   | Required | Description                                                            |
| --------- | ------ | -------- | -------------------------------------------------------------------------- |
| `name`   | string | Yes      | Credential name.                                                       |
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
