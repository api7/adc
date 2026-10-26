# Workflows

Only the release related workflows are described here.

## Release artifacts

`release.yaml` runs on `v*.*.*` tags and uploads archives to the GitHub release. The Rust CLI is the primary implementation and the JavaScript CLI is published under an `adc_js_` prefix.

| Implementation | Archive name | Binary inside |
|---|---|---|
| Rust | `adc_<version>_<os>_<arch>.{tar.gz,zip}` | `adc` |
| Rust debug info | `adc_<version>_<os>_<arch>_debuginfo.{tar.gz,zip}` | `adc.debug` (Linux), `adc.dSYM` (macOS), `adc.pdb` (Windows) |
| JavaScript | `adc_js_<version>_<os>_<arch>.{tar.gz,zip}` | `adc` |

- `<os>` is `linux`, `darwin` or `windows`, `<arch>` is `amd64` or `arm64`.
- The version comes from the tag. The Linux job fails the release when the tag does not match `version` under `[workspace.package]` in `rust/Cargo.toml`, so bump that first.
- Pull requests with the `test/build` label build only the Rust archives and upload them as workflow artifacts.

### Linux glibc floor

- The Linux job builds inside `almalinux:8` (glibc 2.28) so the binary runs on any glibc 2.28 or newer system. The `Check glibc floor` step fails when the binary references a newer `GLIBC_*` symbol version, which can happen after a toolchain or dependency change.
- `adc-linux-smoke` runs the built binary in Debian 11, Debian 12, Ubuntu 22.04 and Ubuntu 24.04 containers for both architectures. It runs after the archives are uploaded, so a failure does not block the upload.
- The binary is not static. Alpine and other musl systems need a glibc compatibility layer, which is not a supported target.
- The Linux job uses a container, so it can only use the JavaScript actions and shell tools available in that image. Install extra tools in the `Install build dependencies` step.

## Docker images

`docker.yaml` builds the images described in [libs/tools/README.md](../../libs/tools/README.md#docker-images). It runs on pushes to `main`, tags and pull requests, but only pushes images on `push` events.
