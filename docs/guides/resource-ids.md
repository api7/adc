# Resource IDs

Every resource ADC manages (services, routes, upstreams, consumers, SSLs, and so on) is identified on the backend by an `id`, not by its `name`. When ADC computes a diff, it matches local resources against remote resources **by `id`**. This choice has a direct consequence:

> If a local resource's `id` does not match the `id` of the resource already on the backend, ADC does not treat it as "the same resource, updated" — it treats it as two different resources, and the diff will **delete** the remote one and **create** a new one from the local definition.

This is why ADC needs a stable, predictable `id` for every resource before it commits a configuration to the control plane: it's the only way `sync` can tell "this is an update to an existing resource" apart from "this is a brand new resource that happens to look similar."

There are two distinct situations, depending on who is managing the resource's lifecycle.

## Resources Fully Managed by ADC

If a resource was created by ADC and is only ever touched through `adc sync`, you never need to write an `id` field yourself. ADC derives it deterministically from the resource's identity, so the same input always produces the same `id`:

| Resource type                              | ID is derived from                                              |
| -------------------------------------------- | ------------------------------------------------------------------ |
| Consumer                                    | The `username` itself (used directly, not hashed).                |
| Global rule / plugin metadata               | The map key (e.g. the plugin name) itself.                        |
| Service, SSL                                | `sha1(name)` — or `sha1(snis.join(','))` for SSL.                  |
| Route, Stream Route, Upstream, Consumer Credential | `sha1("<parent-name>.<name>")`, namespaced by the parent's `name` (e.g. the owning service) so that identically named routes in two different services get different ids. |

This is exactly what `adc dump` (without `--with-id`) reproduces implicitly: because the id is a pure function of the name (and parent name), you don't need to persist it — ADC recomputes the same id every time it reads the file, and it will always match what it previously created on the backend.

The practical implication: **renaming** a resource that ADC generated the id for changes its id too (since the id is derived from the name), which means `sync` will delete the old resource and create a new one under the new name, rather than renaming it in place. If you need to rename a resource without a delete/create cycle, give it an explicit `id` first (see below) so the id stays stable across the rename.

## Adopting Resources Previously Managed by the UI or Admin API

Resources created through the Admin API or a dashboard (API7 Enterprise's UI, for instance) are assigned a random, server-generated `id` — it has no relationship to the resource's `name`. If you write a fresh `adc.yaml` for such a resource and sync it without specifying `id`, ADC computes `sha1(name)`, which will not equal the resource's actual random id already on the backend. The diff sees two different ids and does exactly what's described above: deletes the existing resource and creates a new one in its place. Depending on the resource, this can mean brief traffic disruption, loss of anything keyed to the old id (e.g. external references, audit history), and needless churn.

To bring such a resource under ADC management safely:

1. **Dump the current backend state with real ids inlined:**

   ```bash
   adc dump --with-id -o adc.yaml
   ```

   Every resource in `adc.yaml` now has its actual backend `id` set explicitly. Because every resource type's id resolution is `id ?? generateId(...)` — the explicit `id` always wins over the derived one — ADC will use these ids going forward instead of recomputing them from the name.

2. **Confirm there is no drift before you start managing it declaratively:**

   ```bash
   adc diff -f adc.yaml
   ```

   This should report no changes. If it does report changes, they reflect real differences between what's in `adc.yaml` and the live backend state (for example, fields the dump can't fully round-trip) — resolve those before proceeding.

3. **Commit `adc.yaml` to version control and manage it with `adc sync` from now on.** As long as the `id` field stays in the file, future syncs will match against the correct backend resource and update it in place instead of replacing it.

4. **Keep the `id` field.** Don't strip it out later "for cleanliness" — doing so turns the resource back into a name-derived id on the next sync, which (unless the hash happens to coincide with the real id) triggers the delete/create behavior described above. There's no supported way to migrate a resource from an explicit id back to a derived one without a delete/create cycle.

## Related

- [`adc dump --with-id`](../reference/cli.md#adc-dump)
- [Configuration Reference](../reference/configuration.md)
- [Label Selector](./label-selector.md)
