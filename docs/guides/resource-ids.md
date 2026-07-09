# Resource IDs

ADC matches local resources to remote resources by resource ID. Names are used to generate IDs only when an explicit `id` is not present in the local configuration.

This matters most when adopting resources that already exist on the backend. A resource created through a UI or Admin API may have a server-generated ID that does not match the ID ADC would derive from its name. If ADC cannot match the IDs, it treats the local resource and remote resource as different resources.

## Why IDs Matter

`adc sync` is a reconciliation operation. For each resource in scope, ADC compares the local desired state with the remote state and computes create, update, and delete events.

If an existing remote resource has ID `abc123`, but the local file describes a similar resource with generated ID `def456`, ADC does not treat that as an in-place update. It sees:

- one remote resource to delete
- one local resource to create

Depending on the resource, that replacement can cause traffic disruption, break references that use the old ID, or create unnecessary audit history.

## Generated IDs

When no explicit `id` is set, ADC derives IDs deterministically.

| Resource type       | Generated ID                                    |
| ------------------- | ----------------------------------------------- |
| Consumer            | `username`                                      |
| Global rule         | Plugin name                                     |
| Plugin metadata     | Plugin name                                     |
| Service             | `sha1(name)`                                    |
| SSL                 | `sha1(snis.join(','))`                          |
| Upstream            | `sha1("<service-name>.<upstream-name>")`        |
| Route               | `sha1("<service-name>.<route-name>")`           |
| Stream route        | `sha1("<service-name>.<stream-route-name>")`    |
| Consumer credential | `sha1("<consumer-username>.<credential-name>")` |

For resources that ADC created and continues to manage, you normally do not need to write IDs by hand. ADC will generate the same ID from the same names on every run.

## Rename Behavior

If a resource uses a generated ID, renaming it changes the generated ID. ADC will treat that as deleting the old resource and creating a new one.

To rename a resource without changing its backend ID:

1. Export the resource with explicit IDs.
2. Keep the `id` field in the file.
3. Change the display name or resource name while preserving the `id`.
4. Run `adc diff` and confirm the change is an update, not a delete and create.

## Adopt Existing Resources

Before pointing ADC at resources that were created outside ADC, export the current backend state with IDs:

```bash
adc dump --with-id -o adc.yaml
```

Then confirm that ADC sees no changes:

```bash
adc diff -f adc.yaml
```

If the diff is empty, commit the file and use it as the starting point for ADC-managed configuration.

Keep the exported `id` fields. Removing them later can cause ADC to fall back to generated IDs and replace the resources on the next sync.

## Related

- [Use ADC for Declarative Configuration](./workflow.md)
- [CLI Command Reference](../reference/cli.md#adc-dump)
- [Configuration Reference](../reference/configuration.md)
