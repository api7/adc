# Apache APISIX Backend (Standalone Admin API) for ADC

> **This backend is experimental.**

Apache APISIX introduces an API-driven enhanced standalone mode, which will be used in the Ingress Controller's stateless mode.

This new backend implementation will be added to ADC as the core abstraction layer of Ingress Controller.
This allows it to work on any possible backend type without any special input format adaptation, which will always be the ADC format.

Basically, this is used internally by the Ingress Controller. If you want it to work on its own, you must properly synchronize the configuration to each APISIX instance in the cluster.
This is beyond the scope of the ADC, it needs to be implemented by the client itself, e.g. the Ingress Controller will be responsible for discovering APISIX instances and synchronizing the configuration on each one.

**Therefore, it is not recommended that you use this backend on its own unless you fully understand its principles and how it integrates with APISIX.**

## Supported Features

| Features      | Supported |
| ------------- | --------- |
| Dump to ADC   | ✅         |
| Sync from ADC | ✅         |

## Supported Versions

> Versions not listed below are untested.

| Versions | Supported | Status |
| -------- | --------- | ------ |
| 3.13.x   | ✅         | Full   |

The standalone Admin API was added after the `3.12.0` release, so it requires at least `3.13.0` to use.

## Known Issues/Limitations

### Handling of `conf_version` and `modifiedIndex`

The ADC will ignore the resource-type level `conf_version` and resource level `modifiedIndex` obtained from the API when dumping.
They will not be exported to a YAML file. Updates to conf_version and modifiedIndex will only occur during synchronization.

Their update logic are:

1. Gets the old configuration with `<resources-type>_conf_version` and `modifiedIndex` from API.
2. Converts the configuration back to ADC format for diff checking. It outputs a set of ADC events containing all modifications to indicate which resource should be created, updated or deleted.
3. Use events to override old configurations.
   1. An updated resource will have its modifiedIndex increased;
   2. A created resource's modifiedIndex will be incremented from the `conf_version` of the current resource type;
   3. A deleted resource will have the `conf_version` of the current resource type updated.
4. Recalculates the resource-type level `conf_version`. its rule is: find the maximum value of the old `conf_version` and the `modifiedIndex` in all such resources, and if that value is equal to the old `conf_version`, add one. This means that if there is any resource change (no matter what the operation is), the `conf_version` at the resource-type level will always increase; but the `modifiedIndex` on the resource will only change on the resource that was changed. This minimizes the impact of cache invalidation "noise" on APISIX due to unrelated resource updates.

### Differences in upstream

This backend's upstream converter will not support the configuration of the APISIX health checker or service discovery.

The reason for this is that this backend will primarily be used in an Ingress Controller scenario. In this scenario, the Kubernetes Endpoints mechanism and probes will implement these two capabilities.
