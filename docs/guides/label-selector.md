# Label Selector

`--label-selector` scopes ADC operations to resources with matching labels. Use it when multiple teams, applications, or CI pipelines manage separate parts of the same backend.

Without a selector, `adc sync` reconciles all resources in the command scope. If team A and team B sync separate files to the same backend without partitioning, one team can accidentally delete the other team's resources. A selector prevents that by making each run see only its own labeled resources.

## Basic Usage

```bash
adc sync -f team-a.yaml --label-selector team=a
```

You can provide multiple labels in one flag or repeat the flag:

```bash
adc sync -f catalog.yaml --label-selector team=catalog,env=prod
adc sync -f catalog.yaml --label-selector team=catalog --label-selector env=prod
```

Matching uses exact string equality. Multiple labels are combined with AND, so a resource must match every key-value pair to be included.

ADC does not support wildcards, OR expressions, or key-exists selectors. There is no `ADC_*` environment variable for label selectors; pass them as command-line flags.

## How Remote Filtering Works

For `dump`, and for the remote side of `diff` and `sync`, ADC filters remote resources by label.

Remote filtering applies to top-level resource collections that carry labels, such as:

- `services`
- `consumers`
- `consumer_groups`
- `ssls`
- other label-bearing top-level resource collections supported by the backend

Remote filtering does not apply to `global_rules` or `plugin_metadata`. These resources are keyed by plugin name and are treated as global configuration.

Filtering does not independently select nested resources. If a service matches the selector, its routes and stream routes are included with the service. If a consumer group matches the selector, its nested consumers are included with the group.

For API7 Enterprise, ADC passes label filters to the Admin API so the backend can filter matching top-level resources. For Apache APISIX, ADC fetches the remote configuration and applies the same label filtering client-side.

## How Local Label Injection Works

ADC also applies the selector to the local file. It merges the selector labels into each local top-level resource and selected nested resources. For `diff` and `sync`, this happens before ADC compares the local file with the backend. For `validate`, this happens before ADC sends the local resources for backend validation.

For example, this command:

```bash
adc sync -f catalog.yaml --label-selector team=catalog
```

turns a local service like this:

```yaml
services:
  - name: catalog
    routes:
      - name: list-products
        uris:
          - /products
```

into an in-memory configuration equivalent to:

```yaml
services:
  - name: catalog
    labels:
      team: catalog
    routes:
      - name: list-products
        labels:
          team: catalog
        uris:
          - /products
```

If the file already has the same label key, the selector value wins.

This behavior keeps future runs stable. Resources created under a selector remain visible to that selector, and nested resources do not produce repeated diffs just because the backend copy has labels added by an earlier sync.

## Share One Backend Across Teams

Team A and team B can manage separate files against the same backend:

```bash
# Team A
adc sync -f team-a.yaml --label-selector team=a

# Team B
adc sync -f team-b.yaml --label-selector team=b
```

Team A sees and reconciles only resources labeled `team=a`. Team B sees and reconciles only resources labeled `team=b`.

To inspect one partition:

```bash
adc dump -o team-a.yaml --label-selector team=a
```

## Important Limits

- Do not use label selectors to split ownership of routes inside one shared service. A service is the top-level unit for filtering, so its nested routes move with it.
- Use `--gateway-group` for API7 Enterprise gateway group isolation. Use `--label-selector` when teams share a gateway group or when the backend is Apache APISIX.
- Be careful with `global_rules` and `plugin_metadata`. They are global and are not filtered by label selector.

## Related

- [Resource IDs](./resource-ids.md)
- [CLI Command Reference](../reference/cli.md#common-backend-options)
