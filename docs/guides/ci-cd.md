# Manage Gateway Configuration in CI/CD

Use ADC in a CI/CD pipeline to validate and deploy declarative Apache APISIX or API7 Enterprise configuration. The pipeline checks proposed configuration, shows the expected gateway changes, waits for the required approval, and then reconciles the target gateway with the reviewed files.

The CI/CD runner deploys configuration by running ADC and calling the gateway Admin API. ADC does not watch a Git repository and `adc.yaml` is not a Kubernetes resource, so Argo CD and Flux cannot apply it. If you also use a Kubernetes GitOps controller, run ADC in a separate CI job.

## Prerequisites

- Install ADC on the CI/CD runner or use the published `api7/adc` container image. Pin a released version instead of `latest` or `dev`.
- Store the declarative configuration in version control. See [Use ADC for Declarative Configuration](./workflow.md) to create or adopt an `adc.yaml` file.
- Ensure that the runner can reach the target Admin API.
- Create a dedicated credential for the pipeline. Restrict it to the target gateway group or environment when the backend can scope that identity.
- Decide which resources each pipeline owns before its first deployment. See [Label Selector](./label-selector.md) if more than one team or tool shares a backend.

The examples use ADC `0.29.0` and a file at `gateway/adc.yaml`. Change the version and path to match your repository.

> **Apache APISIX backend:** The ADC Apache APISIX backend is experimental. Although current APISIX releases are tested, some APISIX resources or equivalent configuration forms do not round-trip to an identical representation and can produce a persistent diff. Validate the exact resource types and APISIX version used by your pipeline before adopting automatic production synchronization. See [Apache APISIX backend notes](../../libs/backend-apisix/README.md).

## Run ADC in CI

Install a pinned ADC release on the runner, or run the published image. The image is available from Docker Hub (`api7/adc`) and GitHub Container Registry (`ghcr.io/api7/adc`) for `linux/amd64` and `linux/arm64`.

ADC writes `diff.yaml` into the current working directory, so mount the repository and set the container workdir to that mount. The image entrypoint looks up `main.cjs` relative to the workdir, so override the entrypoint and call `/home/nonroot/main.cjs`.

The image runs as uid `65532`. Do not run it as the host user: `/home/nonroot` is not world-readable, so the binary would be unreachable. Make the mounted directory writable by uid `65532` so ADC can create `diff.yaml` (for example, `chmod a+w .` on a GitHub Actions checkout):

```bash
ADC_IMAGE=api7/adc:0.29.0

docker run --rm \
  -v "${PWD}:/work" \
  -w /work \
  --entrypoint /nodejs/bin/node \
  -e ADC_BACKEND \
  -e ADC_SERVER \
  -e ADC_TOKEN \
  -e ADC_GATEWAY_GROUP \
  "$ADC_IMAGE" \
  /home/nonroot/main.cjs lint -f gateway/adc.yaml
```

Reuse the same `docker run` options for `validate`, `diff`, `sync`, and `dump`. Mount CA and mutual TLS files read-only when those files are not already in the repository:

```bash
-v "/path/to/gateway-ca.pem:/certs/gateway-ca.pem:ro" \
-e ADC_CA_CERT_FILE=/certs/gateway-ca.pem
```

For stronger supply-chain reproducibility, pin the image digest recorded by your artifact policy in addition to the release version.

## Define an Ownership Scope

`adc sync` can create, update, and delete resources in its command scope. A remote resource that is in scope but absent from the local files can be deleted. Do not let independent pipelines reconcile the same unpartitioned backend.

Use a label selector to give one application or team an independent ownership scope:

```bash
adc diff \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production
```

ADC injects the selector labels into the local resources and compares them only with remote resources carrying the same labels. Use the same selector for `validate`, `diff`, `sync`, and scheduled drift checks.

For API7 Enterprise, also set the target gateway group with `ADC_GATEWAY_GROUP` or `--gateway-group`. If you omit it, ADC uses `default`. A gateway group selects the runtime target; a label selector can divide ownership within that group.

> **Note:** ADC adds the `managed-by=adc` label to supported local resources by default, but that label is not used as a default selector. Without `--label-selector`, `adc sync` still reconciles every resource in the command scope, including resources created outside ADC. Use an explicit selector whenever a backend or gateway group contains resources owned by another team or tool.

Label selectors operate on top-level resources. They cannot divide ownership of routes nested in the same service, and they do not scope `global_rules` or `plugin_metadata`. See [Label Selector](./label-selector.md) before sharing a backend between pipelines.

## Store Connection Settings Securely

Configure backend connection settings as protected CI/CD secrets instead of committing a `.env` file:

| Setting             | Apache APISIX | API7 Enterprise               |
| ------------------- | ------------- | ----------------------------- |
| `ADC_BACKEND`       | `apisix`      | `api7ee`                      |
| `ADC_SERVER`        | Admin API URL | API7 Enterprise Admin API URL |
| `ADC_TOKEN`         | Admin API key | Dashboard API token           |
| `ADC_GATEWAY_GROUP` | Not used      | Target gateway group          |

Use a trusted CA certificate with `ADC_CA_CERT_FILE` when the endpoint uses a private certificate authority. Store a client certificate and key in `ADC_TLS_CLIENT_CERT_FILE` and `ADC_TLS_CLIENT_KEY_FILE` if the endpoint requires mutual TLS. Do not use `ADC_TLS_SKIP_VERIFY` in a production pipeline.

Do not expose write-capable credentials to workflows triggered from untrusted forks. Run local lint checks without secrets for every pull request, and restrict backend validation, planning, and deployment jobs to trusted code and protected environments.

## Check Pull Requests

Run `lint` for every proposed change. It verifies ADC syntax and schema rules without connecting to a backend:

```bash
adc lint -f gateway/adc.yaml
```

For trusted pull requests, also produce a diff against a non-production backend. Run `validate` first when the backend supports it:

```bash
adc validate \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=staging

adc diff \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=staging
```

`validate` asks the backend to check the proposed resources without applying them. It requires API7 Enterprise 3.9.10 or later, or Apache APISIX 3.17 or later. On older APISIX versions the Admin API has no `/apisix/admin/configs/validate` endpoint and the command fails; skip `validate` then. `diff` prints a summary and writes the complete machine-readable plan to `diff.yaml`. Upload `diff.yaml` as a CI artifact so reviewers can inspect creates, updates, and deletions.

ADC has no apply-plan command. Reviewers inspect `diff.yaml`; they do not apply that file. `adc diff` also exits successfully when it finds differences. If a policy requires the job to fail on drift, inspect `diff.yaml` explicitly as shown in [Detect Drift](#detect-drift).

## Plan and Deploy a Change

Keep planning and deployment as separate jobs. The plan job publishes `diff.yaml`. Include `validate` in that job when the backend supports it. The deployment job should require environment approval, recalculate the diff against the latest backend state, and then synchronize the reviewed configuration.

Use the same ADC version, files, backend, gateway group, resource filters, and label selector in both jobs. If any of these inputs differ, the approved plan does not describe the deployment scope.

### Plan

Omit `validate` when the backend does not implement it.

```bash
adc validate \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production

adc diff \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production
```

Review `diff.yaml`, paying particular attention to `delete` events. A large or unexpected deletion usually means that a file, selector, gateway group, or resource filter does not match the intended ownership scope.

### Deploy

After approval, recalculate the plan from the reviewed files. Compare that live plan with the approved artifact, or re-review it, before `sync`. If the live plan contains unexpected operations, stop and run the plan job again instead of synchronizing a different change set:

```bash
adc validate \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production

adc diff \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production

adc sync \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production
```

Serialize deployments that target the same ownership scope. For example, a GitHub Actions deployment workflow can use:

```yaml
concurrency:
  group: adc-production-catalog
  cancel-in-progress: false
```

Do not cancel a deployment after `adc sync` has started. ADC sends resource operations through the Admin API and does not apply the entire plan as one atomic transaction. Concurrent requests can succeed before a later request fails. If a runner or request fails partway through, preserve the logs, correct the failure, and run the same desired configuration again to converge the backend.

Use `--request-concurrent` to reduce request concurrency when the Admin API is rate limited. It changes request parallelism, not the ownership or deletion scope.

## Verify a Deployment

Run the same diff immediately after synchronization:

```bash
adc diff \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production
```

For configuration that round-trips without backend normalization, `diff.yaml` should contain an empty list:

```yaml
[]
```

If the same diff remains after a successful sync, inspect whether the backend populated defaults or normalized an equivalent configuration form. Align the source file with a stable ADC representation when possible, and test the result again. Do not enable a failing drift job, and do not suppress repeated events blindly: a persistent diff can also indicate a failed operation or real drift. Some Apache APISIX versions fill defaults such as route `priority` and upstream `hash_on`; see [Apache APISIX backend notes](../../libs/backend-apisix/README.md). Test the exact APISIX version your pipeline uses before treating an empty `diff.yaml` as the expected post-sync result.

Then run application-level smoke tests through the gateway. ADC verifies and reconciles gateway configuration, but it does not prove that upstream applications, DNS, certificates, or external dependencies behave as expected.

## Detect Drift

After confirming that the desired configuration produces a stable empty diff, run a scheduled `diff` with the same target and ownership settings as the deployment job. The following check fails when `diff.yaml` contains one or more events:

```bash
adc diff \
  -f gateway/adc.yaml \
  --label-selector team=catalog,env=production

if [ "$(tr -d '[:space:]' < diff.yaml)" != "[]" ]; then
  echo "Gateway configuration drift detected. Review diff.yaml."
  exit 1
fi
```

Upload `diff.yaml` even when the job fails. Investigate whether the difference came from an intentional emergency change, another automation system, an incorrect ownership scope, or a failed deployment. Do not automatically overwrite unexplained production drift before reviewing it.

## Roll Back

Gateway configuration should be rolled back from the same version-controlled source of truth:

1. Revert the configuration commit or select a previously approved revision.
2. Run `lint` and `diff` against the target backend. Run `validate` when the backend supports it.
3. Review the rollback plan for destructive changes.
4. Run `sync` with the same ownership scope used for deployment.
5. Confirm that `diff.yaml` is empty and repeat the application smoke tests.

Keep exported backups when you adopt existing resources or when your operational policy requires an independent recovery artifact:

```bash
adc dump \
  --with-id \
  --label-selector team=catalog,env=production \
  -o gateway-backup.yaml
```

Treat a dump as sensitive configuration. Store it in an access-controlled artifact location and define a retention policy. Dumped files can include empty `global_rules` and `plugin_metadata` maps. Those resources are not limited by `--label-selector`, so do not sync a dump back until you have removed keys you do not own.

## Map the Model onto Your CI System

Provider YAML differs, but the job boundaries should not:

- Every pull request: `lint` only, with no Admin API credentials.
- Trusted code only: `diff` against a non-production backend (and `validate` when supported), then upload `diff.yaml`.
- Protected production environment: recalculate the plan, review unexpected operations, then `sync`.
- One concurrency group per ownership scope for jobs that can call `sync`. Do not cancel an in-progress sync when a newer job is queued.
- Scheduled drift detection: `diff` only. Do not grant that job permission to `sync`.

## Related

- [Use ADC for Declarative Configuration](./workflow.md)
- [Resource IDs](./resource-ids.md)
- [Label Selector](./label-selector.md)
- [CLI Command Reference](../reference/cli.md)
