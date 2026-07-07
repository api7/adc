# Label Selector

`--label-selector` lets multiple ADC configuration sources — different teams, different CI pipelines, different config files — safely manage disjoint subsets of resources on the **same** backend, without one sync accidentally deleting another's resources.

## The Problem It Solves

`adc sync` and `adc diff` are full reconciliation operations: whatever exists on the backend but is absent from your local file(s) is treated as "should be deleted." That's fine as long as one ADC pipeline owns the entire backend. It breaks down the moment two teams share a backend — team A's `sync` would see team B's resources as unmanaged leftovers and delete them.

`--label-selector` fixes this by scoping both sides of the reconciliation to only the resources that carry a given label:

```bash
adc sync -f team-a.yaml --label-selector team=a
```

This complements (it doesn't replace) API7 Enterprise's `--gateway-group`, which is a hard, backend-level partition available only for the `api7ee` backend. Label selectors are a soft, convention-based partition that works the same way on every backend, including plain `apisix`.

## Syntax and Matching Rules

```bash
--label-selector team=a
--label-selector team=a,env=prod        # comma-separated pairs in one flag
--label-selector team=a --label-selector env=prod   # or repeat the flag
```

- Matching is **exact string equality**, and multiple pairs are combined with **AND** — a resource must match every key=value pair to be included. There's no "key exists regardless of value," no wildcard, no OR.
- There is no `ADC_*` environment variable for this option — it's flag-only.

## What Actually Happens on Each Side

`--label-selector` does two different things depending on whether it's applied to the local file or the remote backend state, and understanding both is necessary to use it correctly.

### Remote: Filtering

When ADC fetches the backend configuration (`dump`, and the remote side of `diff`/`sync`), it drops any resource whose `labels` don't match every pair in the selector. This only happens at the **top-level resource** granularity — `services`, `consumers`, `ssls`. `global_rules` and `plugin_metadata` are never filtered (they're global singletons keyed by plugin name, not label-bearing per-resource collections).

Critically, filtering does **not** recurse into nested resources. A service's `routes`/`stream_routes` and a consumer's `credentials` travel with their parent as soon as the parent matches — they are never independently checked against the selector. This is deliberate; `libs/backend-api7/src/fetcher.ts` has an explicit comment about it:

```
// In the current design, the consumer's credentials are not filtered
// using labels because nested labels filters can be misleading. Even
// if labels set for the consumer, the labels filter is not attached.
```

The `api7ee` backend applies this filter server-side (as `labels[key]=value` Admin API query parameters), so a filtered `dump` doesn't transfer the whole backend configuration over the wire. The `apisix` backend has no such API-level filter, so ADC always fetches the entire remote configuration and applies the same filtering client-side afterward. Either way, the CLI performs the same client-side filtering pass again regardless of backend, so behavior is consistent even though the `apisix` path is less efficient.

### Local: Injection

When ADC loads your local file(s) for `sync`/`diff`, it does the opposite of filtering: it **merges** every key=value pair from the selector into the `labels` of every top-level resource, and recurses into a service's `routes`/`stream_routes` to stamp them too (existing label keys with the same name are overwritten by the selector's value). Nothing local is ever dropped or excluded — `--label-selector` only removes resources from the *remote* side of the comparison.

This injection isn't cosmetic — it's necessary for two different reasons depending on the level:

**On top-level resources, it's required for correctness.** The exact same selector is used to filter the remote side on every future run. If a resource were created without the label, the very next invocation with the same `--label-selector` would no longer see it as "already there" (since remote filtering would exclude it) — the diff would try to create it again against an identical, deterministic id (see [Resource IDs](./resource-ids.md)), producing a conflict or a resource that's permanently invisible to its own partition. Injection guarantees anything synced under a given selector stays discoverable under that same selector.

**On nested routes/stream_routes, it's only about diff stability, not filtering correctness.** Since nested resources are never independently filtered (see above), an unlabeled route wouldn't fall out of scope. But the remote route already carries the label — stamped there when it was first created, as a child of an already-labeled service — so if the local copy doesn't carry the same label, the differ sees a real (if meaningless) field difference every time and reports a spurious `update route` event on every single `sync`/`diff`, forever.

A side effect of both: you don't have to hand-write `labels: { team: a }` on every resource in your file. Declare it once via the flag (or wire it into your CI pipeline per team/environment), and ADC applies it uniformly — removing the chance that a forgotten label on one resource silently drops it out of its partition.

## Example: Two Teams, One Backend

Team A and team B each maintain their own configuration file and CI pipeline against the same `apisix`/`api7ee` instance:

```bash
# Team A's pipeline
adc sync -f team-a.yaml --label-selector team=a

# Team B's pipeline
adc sync -f team-b.yaml --label-selector team=b
```

Team A's `sync` only ever sees (and can therefore only ever delete or update) resources labeled `team=a`; team B's resources are invisible to it, and vice versa. Neither file needs to mention the `team` label explicitly — ADC stamps it on every resource each pipeline creates.

To inspect what a given partition currently looks like on the backend:

```bash
adc dump -o team-a-backup.yaml --label-selector team=a
```

## Related

- [CLI Command Reference](../reference/cli.md#common-backend-options)
- [Resource IDs](./resource-ids.md)
