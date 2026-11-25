# API7 Enterprise Backend for ADC

## Supported Features

| Features      | Supported |
| ------------- | --------- |
| Dump to ADC   | ✅         |
| Sync from ADC | ✅         |

## Supported Versions

| Versions | Supported |
| -------- | --------- |
| 3.5.5    | ✅         |
| 3.6.1    | ✅         |
| 3.7.8    | ✅         |
| 3.8.23   | ✅         |
| 3.9.14   | ✅         |
| 3.10.1   | ✅         |

## Running E2E tests locally

Export `API7_ADMIN_INITIAL_PASSWORD` before starting the dashboard and running
either E2E suite. For a development image, generate a strong initial password
with `export API7_ADMIN_INITIAL_PASSWORD="Aa1!$(openssl rand -hex 24)"`. For
the older releases pinned in the E2E matrix, use the image's factory login
password.

For Rust tests that bootstrap their own token, also generate
`export BACKEND_API7_ROTATED_ADMIN_PASSWORD="Aa1!$(openssl rand -hex 24)"`
once per dashboard instance. Keep it in the same environment for subsequent
test processes so they can sign in after the initial password is rotated.
