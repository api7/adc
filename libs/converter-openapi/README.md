# OpenAPI Converter

A library for implementing the OpenAPI specification and ADC profile conversion.

| Direction      | Supported |
| -------------- | --------- |
| OpenAPI to ADC | ✅         |
| ADC to OpenAPI | ❎         |

## Supported Extension Fields

| Supported Levels                                                                          |
| ----------------------------------------------------------------------------------------- |
| Root Level: on the root of the OAS document                                               |
| Path Level: on each path object                                                           |
| Operation Level: on each HTTP method object for each path                                 |
| Server Level: on each item in the servers field, supports Root, Path and Operation levels |

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
      <td rowspan="3">x-adc-labels</td>
      <td>Root Level</td>
      <td rowspan="3">Add labels field to the specified level. It supports string and string array formats.</td>
    </tr>
    <tr>
      <td>Path Level</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
    <tr>
      <td rowspan="3">x-adc-plugins</td>
      <td>Root Level</td>
      <td>Add plugins field to the specified level. It is an object that contains one or more plugins.</td>
    </tr>
    <tr>
      <td>Path Level</td>
      <td rowspan="2">Plugin objects at the Path level and Operation level will cause the service to be split, i.e. the sub-level containing the plugin will be included in a new service.</td>
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
      <td rowspan="3">It supports setting/overriding parameters in the route at various levels. This field on sub-levels will cause the service to be split.</td>
    </tr>
    <tr>
      <td>Path Level</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
    <tr>
      <td rowspan="2">x-adc-route-uris</td>
      <td>Path Level</td>
      <td rowspan="2">Append additional URIs to the route. Accepts a string array of URI patterns. The original path-based URI is preserved, and additional URIs are appended to the route's uris array. Operation level takes precedence over Path level.</td>
    </tr>
    <tr>
      <td>Operation Level</td>
    </tr>
  </tbody>
</table>

## Extension Fields processing logic

### `x-adc-plugins` and `x-adc-plugin-[plugin-name]`

> They can be set at all three main levels, root, path, and operation.

#### Set both `x-adc-plugins` and `x-adc-plugin-[plugin-name]` at the same level

1. For plugins with non-same names, their configurations will be merged.

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

1. For plugins with same names, the configurations in `x-adc-plugin-[plugin-name]` will override the one in `x-adc-plugins`.

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

#### Set both `x-adc-plugins` or `x-adc-plugin-[plugin-name]` in multiple levels

- Plugin configurations at the root level will be mapped to the exported service.

- Both the path level and the operation level will be mapped to the routes included in this service.

The difference is:

- The plugins on the path level will be included on all the routes corresponding to the method for that path.

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

- The plugin with the same name at the operation level as at the path level will override the one at the path.

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
