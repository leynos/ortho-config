# test_helpers

Shared test-only helper utilities used by crates in the `ortho-config`
workspace.

Published package name: `ortho_config_test_helpers`.

This crate provides:

- Deprecated (since 0.11.0) RAII guards for process-global environment variable
  mutation. Inject an `ortho_config::MapEnv` instead.
- Deprecated (since 0.11.0) RAII guards for current working directory mutation.
  Pin the directory with `MapEnv::with_current_dir` instead.
- Shared text normalization functions for behavioural tests.

## Intended usage

This crate is intended for test targets (`[dev-dependencies]`). It is not
intended as a runtime dependency for production binaries or libraries.

## Publishing

`ortho_config` and other workspace crates depend on this crate as a versioned
dependency during `cargo publish` verification. Publish
`ortho_config_test_helpers` before publishing crates that rely on it.
