# OpenAPI Converter Reference

`adc convert openapi` converts an OpenAPI 2.0 (Swagger) or OpenAPI 3.x specification (JSON or YAML) into ADC configuration. See the [CLI Command Reference](./cli.md#adc-convert-openapi) for the command's flags and basic usage.

| Direction      | Supported |
| -------------- | --------- |
| OpenAPI to ADC | ✅        |
| ADC to OpenAPI | ❎        |

A plain OpenAPI document only describes endpoints — it has no notion of ADC-specific concepts like plugins, labels, or upstream defaults. To configure those on the converted output, annotate the specification with `x-adc-*` extension fields.

## Supported Extension Fields

Extensions are recognized at up to four levels of a specification:

- **Root level**: the root of the OAS document. Applies to the entire generated service.
- **Path level**: on each path object. Applies to every operation (HTTP method) under that path.
- **Operation level**: on each HTTP method object within a path. Applies to that specific route.
- **Server level**: on each item of a `servers:` field, which can itself appear at the root, path, or operation level. Applies to the upstream node(s) derived from that server entry. Only meaningful for OpenAPI 3.x — OpenAPI 2.0 (Swagger) has no `servers:` field.

<table>
  <thead>
    <tr>
      <th>Field</th>
      <th>Level</th>
      <th>Description</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td rowspan="2">x-adc-name</td>
      <td>Root Level</td>
      <td>Override the name of the generated main service</td>
    </tr>
    <tr>
      <td>Operation Level</td>
      <td>Override the name of the generated route</td>
    </tr>
    <tr>
      <td rowspan="2">x-adc-labels</td>
      <td>Root Level</td>
      <td rowspan="2">Add labels field to the specified level. It supports string and string array formats.</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
    <tr>
      <td rowspan="3">x-adc-plugins</td>
      <td>Root Level</td>
      <td rowspan="3">Add plugins field to the specified level. It is an object that contains one or more plugins. Plugins set at the path or operation level are attached directly to the routes generated from that path or operation; they do not by themselves cause the service to be split.</td>
    </tr>
    <tr>
      <td>Path Level</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
    <tr>
      <td rowspan="3">x-adc-plugin-[plugin-name]</td>
      <td>Root Level</td>
      <td rowspan="3">It will be consistent with x-adc-plugins. However, those configured using this format will override plugins of the same name in x-adc-plugins.</td>
    </tr>
    <tr>
      <td>Path Level</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
    <tr>
      <td rowspan="3">x-adc-service-defaults</td>
      <td>Root Level</td>
      <td rowspan="3">It supports setting/overriding parameters in the service at various levels. This field on sub-levels will cause the service to be split.</td>
    </tr>
    <tr>
      <td>Path Level</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
    <tr>
      <td rowspan="3">x-adc-upstream-defaults</td>
      <td>Root Level</td>
      <td rowspan="3">It supports setting/overriding parameters in the upstream at various levels. This field on sub-levels will cause the service to be split.</td>
    </tr>
    <tr>
      <td>Path Level</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
    <tr>
      <td rowspan="3">x-adc-upstream-node-defaults</td>
      <td>Root Level - Server Level</td>
      <td rowspan="3">It supports setting/overriding parameters in the upstream's node at various levels. The servers field on sub-levels will cause the service to be split.<br>

```yaml
servers:
  - url: "https://httpbin.org"
    x-adc-upstream-node-defaults:
      priority: 100
  - url: "http://httpbin.org"
    x-adc-upstream-node-defaults:
      priority: 100
```

</td>
    </tr>
    <tr>
      <td>Path Level - Server Level</td>
    </tr>
    <tr>
      <td>Operation Level - Server Level</td>
    </tr>
    <tr>
      <td rowspan="3">x-adc-route-defaults</td>
      <td>Root Level</td>
      <td rowspan="3">It supports setting/overriding parameters in the route at various levels. Sub-level values override root-level values (operation > path > root); the merged result is applied to each route. This field does not by itself cause the service to be split.</td>
    </tr>
    <tr>
      <td>Path Level</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
  </tbody>
</table>

Setting `x-adc-service-defaults`, `x-adc-upstream-defaults`, or a sub-level `servers:`/`x-adc-upstream-node-defaults` entry on a specific path or operation causes that operation to be moved into its own separate service in the output, because per-route upstream or service configuration cannot live on a service shared with other routes. `x-adc-plugins`/`x-adc-plugin-[plugin-name]`, `x-adc-labels`, `x-adc-route-defaults`, and `x-adc-name` are applied in place and do not split the service.

## Extension Fields Processing Logic

### `x-adc-plugins` and `x-adc-plugin-[plugin-name]`

They can be set at all three main levels: root, path, and operation.

#### Set both `x-adc-plugins` and `x-adc-plugin-[plugin-name]` at the same level

1. For plugins with different names, their configurations are merged.

<table>
  <tr>
    <th>Input</th>
    <th>Output</th>
  </tr>
  <tr>
<td>

```yaml
x-adc-plugins:
  test-plugin1:
    key: value
x-adc-plugin-test-plugin2:
  key: value
```

</td>
<td>

```yaml
plugins:
  test-plugin1:
    key: value
  test-plugin2:
    key: value
```

</td>
  </tr>
</table>

2. For plugins with the same name, the configuration in `x-adc-plugin-[plugin-name]` overrides the one in `x-adc-plugins`.

<table>
  <tr>
    <th>Input</th>
    <th>Output</th>
  </tr>
  <tr>
<td>

```yaml
x-adc-plugins:
  test-plugin1:
    key: value
x-adc-plugin-test-plugin1:
  key1: value1
```

</td>
<td>

```yaml
plugins:
  test-plugin1:
    key1: value1
```

</td>
  </tr>
</table>

#### Set `x-adc-plugins` or `x-adc-plugin-[plugin-name]` at multiple levels

- Plugin configurations at the root level are mapped to the exported service.
- Both the path level and the operation level are mapped to the routes included in that service.

The difference between the two:

- Plugins at the path level are included on every route generated for that path (i.e. for every HTTP method under it).

<table>
  <tr>
    <th>Input</th>
    <th>Output</th>
  </tr>
  <tr>
<td>

```yaml
...
paths:
  /anything:
    x-adc-plugin-test-plugin:
      key: value
    get: ...
    post: ...
```

</td>
<td>

```yaml
...
services:
  - name: demo
    routes:
      - name: demo_anything_get
        uris: [/anything]
        methods: [GET]
        plugins:
          test-plugin:
            key: value
      - name: demo_anything_post
        uris: [/anything]
        methods: [POST]
        plugins:
          test-plugin:
            key: value
```

</td>
  </tr>
</table>

- A plugin with the same name at the operation level overrides the one at the path level, for that operation only.

<table>
  <tr>
    <th>Input</th>
    <th>Output</th>
  </tr>
  <tr>
<td>

```yaml
...
paths:
  /anything:
    x-adc-plugin-test-plugin:
      key: value
    get: ...
      x-adc-plugin-test-plugin:
        key1: value1
      x-adc-plugin-test-plugin2:
        key: value
    post: ...
```

</td>
<td>

```yaml
...
services:
  - name: demo
    routes:
      - name: demo_anything_get
        uris: [/anything]
        methods: [GET]
        plugins:
          test-plugin:
            key1: value1
          test-plugin2:
            key: value
      - name: demo_anything_post
        uris: [/anything]
        methods: [POST]
        plugins:
          test-plugin:
            key: value
```

</td>
  </tr>
</table>

## Full Example

The specification below sets root-level labels and a `key-auth` plugin, plus operation-level defaults that cause the `GET /anything/*` route to be split into its own service:

```yaml title="openapi.yaml"
openapi: 3.0.0
info:
  title: httpbin API
  description: httpbin API example.
  version: 1.0.0
servers:
  - url: 'http://httpbin.org:80'
x-adc-labels:
  server: production
  api: httpbin
x-adc-plugins:
  key-auth:
    _meta:
      disable: false
paths:
  /anything/*:
    get:
      summary: Returns anything that is passed into the request.
      x-adc-name: httpbin-anything
      x-adc-service-defaults:
        path_prefix: /api
      x-adc-upstream-defaults:
        timeout:
          connect: 10
          send: 10
          read: 10
      responses:
        '200':
          description: Successful Response
          content:
            application/json:
              schema:
                type: string
```

Running `adc convert openapi -f openapi.yaml -o adc.yaml` produces:

```yaml title="adc.yaml"
services:
  - description: httpbin API example.
    labels:                              # inherited from root x-adc-labels
      api: httpbin
      server: production
    name: httpbin-API_anything*_get      # auto-generated from title + path + method
    plugins:                             # inherited from root x-adc-plugins
      key-auth:
        _meta:
          disable: false
    routes:
      - description: Returns anything that is passed into the request.
        methods:
          - GET
        name: httpbin-anything           # from operation x-adc-name
        uris:
          - /api/anything/*              # path_prefix from x-adc-service-defaults is
                                          # inlined into the URI for APISIX compatibility
    upstream:
      nodes:
        - host: httpbin.org              # derived from servers[0].url
          port: 80
          weight: 100
      pass_host: pass
      scheme: http
      timeout:                           # from operation x-adc-upstream-defaults
        connect: 10
        read: 10
        send: 10
```

## Related

- [CLI Command Reference](./cli.md#adc-convert-openapi)
- [ADC Workflow Guide](../guides/workflow.md)
