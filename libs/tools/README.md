# tools

Distribution assets that are not part of the CLI itself.

## Docker images

Both images are published to `docker.io/api7/adc` and `ghcr.io/api7/adc` by `.github/workflows/docker.yaml`, for `linux/amd64` and `linux/arm64`.

| Dockerfile | Implementation | Tags |
|---|---|---|
| `src/docker/Dockerfile` | Rust | `<version>`, `dev` |
| `src/docker/Dockerfile.js` | JavaScript | `<version>-js`, `dev-js` |

- The Rust image builds `adc-cli` in `rust:1-trixie` and runs it on `gcr.io/distroless/cc-debian13:nonroot`. The runtime image provides glibc, libgcc and CA certificates, so the binary must stay glibc based. Moving to a static musl binary would require changing the base image as well.
- The Docker build context is the repository root. `.dockerignore` excludes `rust/target` and `node_modules`, keep them excluded or the context becomes very large.
- The entrypoint of each image is the CLI binary itself, so arguments are passed straight to `adc`.

## Install script

`src/s3/install` is what `https://run.api7.ai/adc/install` serves. `.github/workflows/update-s3-and-cloudfront.yaml` syncs it to S3 on every merge to `main`, independent of any release tag.

- It downloads `adc_<version>_<os>_<arch>.tar.gz` from the GitHub release, which is the Rust build. The file names are defined in [.github/workflows/README.md](../../.github/workflows/README.md#release-artifacts), keep both in step.
- The architecture mapping only converts `x86_64` to `amd64`. On Linux arm64 `uname -m` reports `aarch64`, which is not mapped.
