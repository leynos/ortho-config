# Migration guide: v0.8.0 to v0.9.0

## Table of contents

- [Introduction](#introduction)
- [At-a-glance breaking changes](#at-a-glance-breaking-changes)
- [1. Update versions](#1-update-crate-versions)
- [2. Nothing to change for ambient discovery](#2-nothing-to-change-for-ambient-discovery)
- [3. Inject `MapEnv` for hermetic tests](#3-inject-mapenv-for-hermetic-tests)
- [4. Observe discovery telemetry](#4-observe-discovery-telemetry)
- [5. Enable the optional `metrics` feature](#5-enable-the-optional-metrics-feature)
- [6. Redaction contract for upgraders](#6-redaction-contract-for-upgraders)

## Introduction

This guide describes how to upgrade applications from `ortho-config` v0.8.0 to
v0.9.0. The release is additive: configuration discovery now reads the
environment through an injectable [`EnvSource`], rather than always reading
`std::env` directly, and emits structured `tracing` events (plus optional
`metrics` counters) describing its decisions. Existing callers of
`ConfigDiscovery` see no behavioural change unless they opt in to the new
`env_source(...)` builder method or the `metrics` feature.

## At-a-glance breaking changes

| Area | Impact | Section |
| ----------------------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------- |
| Core API | No breaking changes; the default `ConfigDiscovery` behaviour is unchanged. | [2](#2-nothing-to-change-for-ambient-discovery) |
| Test helpers | New opt-in `env_source(...)` builder method and `MapEnv` type for hermetic discovery tests. | [3](#3-inject-mapenv-for-hermetic-tests) |
| Observability | New `tracing` events at `DEBUG` level describe every discovery decision. | [4](#4-observe-discovery-telemetry) |
| Optional dependency | The new `metrics` feature is off by default and adds the `metrics` crate as an optional dependency. | [5](#5-enable-the-optional-metrics-feature) |

_Table 1: Summary of v0.9.0 changes and where to read more._

## 1. Update crate versions

### Before: v0.8.0 dependencies

```toml
ortho_config = { version = "0.8.0", features = ["yaml"] }
ortho_config_macros = "0.8.0"
```

### After: v0.9.0 dependencies

```toml
ortho_config = { version = "0.9.0", features = ["yaml"] }
ortho_config_macros = "0.9.0"
```

Update every `ortho_config` and `ortho_config_macros` dependency to `0.9.0`.
No other feature-flag changes are required unless you adopt the optional
`metrics` feature described below.

## 2. Nothing to change for ambient discovery

`ConfigDiscovery` reads several environment variables during discovery: the
configuration-path selector named by `env_var`, the XDG base directories, the
Windows application-data folders, and `HOME`/`USERPROFILE`. Previously these
came straight from `std::env`. They now flow through an [`EnvSource`] trait,
but the default source, `process_env_source()`, wraps the live process
environment and preserves the existing behaviour exactly — including the
platform home-directory fallback via `dirs::home_dir()` when neither `HOME`
nor `USERPROFILE` is set.

Consumers that build `ConfigDiscovery` without calling the new
`env_source(...)` method require no changes at all.

## 3. Inject `MapEnv` for hermetic tests

`ConfigDiscoveryBuilder::env_source(...)` accepts a `SharedEnvSource`
(`Arc<dyn EnvSource>`) and lets tests supply a fixed set of variables instead
of mutating the real process environment. Because the values live on the
`MapEnv` instance rather than in global state, tests using distinct `MapEnv`
values are independent and may run concurrently without a serializing lock.

```rust,no_run
use ortho_config::{ConfigDiscovery, MapEnv};
use std::sync::Arc;

let env = Arc::new(MapEnv::new().with_var("DEMO_CONFIG", "/etc/demo.toml"));
let discovery = ConfigDiscovery::builder("demo")
    .env_var("DEMO_CONFIG")
    .env_source(env)
    .build();

assert!(discovery.candidates().iter().any(|p| p.ends_with("demo.toml")));
```

`MapEnv` also offers `insert`, `remove`, and `FromIterator<(K, V)>` for
building or mutating a fixture in place. Lookup remains by name only — there
is deliberately no way to enumerate an `EnvSource`, so injecting one never
risks exposing unrelated variables a test fixture happens to hold.

Two points are easy to miss when adopting this:

- **Scope.** An injected source controls file _discovery_ only — the
  `env_var` selector, XDG/Windows base directories, and home resolution. It
  does not affect the `APP_*` configuration-value merge layer (`CsvEnv`,
  wrapping `figment`'s `Env` provider), which still reads the process
  environment. Injecting that layer is tracked separately by
  [issue #412](https://github.com/leynos/ortho-config/issues/412).
- **Home fallback.** `EnvSource::home_fallback` defaults to `None`, so a
  `MapEnv` supplying neither `HOME` nor `USERPROFILE` yields no home
  candidate at all, rather than falling back to the host's real home
  directory. This keeps a test's candidate list independent of the machine it
  runs on. Implement `home_fallback` on a custom `EnvSource` if you need a
  fallback for that source.

Implement `EnvSource` directly (it is object-safe, `fmt::Debug + Send +
Sync`) for anything richer than a fixed map.

## 4. Observe discovery telemetry

Discovery now emits `tracing` events at `DEBUG` level at each decision point:
which environment source was selected (`discovery.source_selected`), how the
configuration-path selector resolved (`discovery.selector`), which variables
supplied the XDG/Windows base directories (`discovery.xdg`), how the home
directory was resolved (`discovery.home`), when an operation starts
(`discovery.attempt`), when an individual candidate is rejected
(`discovery.candidate`), and the terminal outcome of the operation
(`discovery.load`). Attach any `tracing` subscriber at `DEBUG` level to see
them; no configuration is required to enable emission.

This is purely additive — no existing behaviour changes — but it is useful
context when diagnosing "why did discovery load _that_ file" during an
upgrade. See the [redaction contract](#6-redaction-contract-for-upgraders)
below for what these events do and do not carry.

## 5. Enable the optional `metrics` feature

Enabling the new `metrics` feature emits three counters through the
[`metrics`](https://docs.rs/metrics) facade, each with its own label set:

- `ortho_config.discovery.attempts` — labelled with the `operation`.
- `ortho_config.discovery.outcomes` — labelled with the `operation` and the
  `outcome`.
- `ortho_config.discovery.candidate_failures` — labelled with the
  `operation`, the candidate's bounded `source`, and its error `category`.

```toml
[dependencies]
ortho_config = { version = "0.9.0", features = ["metrics"] }
```

The feature is off by default: a library should not choose a metrics backend
for the application embedding it, and the facade records nothing until that
application installs a recorder — `ortho_config` never installs one itself.
The `tracing` events described above are emitted regardless of whether this
feature is enabled.

## 6. Redaction contract for upgraders

Both the `tracing` events and the `metrics` labels are drawn from a closed,
fixed vocabulary — values such as `accepted`, `empty`, `unset`, `not_found`,
or bounded source/category names. They never carry environment variable
values, resolved filesystem paths, or file contents; the events describe the
_decision_, never the datum it was made from. `Debug` output for
`ConfigDiscoveryBuilder` and `MapEnv` follows the same discipline: it reports
counts (for example, how many variables a `MapEnv` holds, or how many project
roots a builder has) and the injected-versus-process distinction, never the
underlying paths or values.

This guarantee covers OrthoConfig's own event fields: they exclude
environment values, resolved paths, and file contents, so forwarding them
unmodified from a process holding secrets in its environment needs no extra
redaction step. It does not extend to fields a subscriber adds afterwards —
span context, request identifiers, or other attributes attached before
forwarding remain subject to the subscriber's normal redaction policy. Pair
telemetry with `ConfigDiscovery::candidates()` when a specific path is
required for debugging, since the telemetry itself will not name it.

[`EnvSource`]: https://docs.rs/ortho_config/latest/ortho_config/trait.EnvSource.html
