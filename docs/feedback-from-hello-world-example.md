# Mitigating Boilerplate in OrthoConfig: Detailed Improvement Proposals

Based on the `hello_world` example in the attached `ortho-config` repository,
here are detailed proposals to make the library more expressive and flexible.
Each proposal addresses a specific area of boilerplate identified in the
example and suggests changes aimed at **implementing developers** of
`ortho-config`. Code references are provided from the repository to illustrate
current behavior and guide the proposed enhancements.

## 1. Abstracted Configuration File Discovery

**Issue:** The example contains a lot of repetitive, platform-specific code to
find configuration files. In `examples/hello_world/src/cli/discovery.rs`,
functions like `add_xdg_config_paths`, `add_windows_config_paths`, and others
are manually invoked to build a list of candidate
paths([1](examples/hello_world/src/cli/discovery.rs#L14-L23)).
 This includes checking an explicit environment variable
(`HELLO_WORLD_CONFIG_PATH`) for a config file and searching standard locations
(XDG directories, `%APPDATA%`, home directory, working
directory)([1](examples/hello_world/src/cli/discovery.rs#L29-L37))([1](examples/hello_world/src/cli/discovery.rs#L68-L76)).
 Writing and maintaining this logic is tedious and error-prone.

**Proposal:** Integrate a **cross-platform configuration discovery utility**
into `ortho-config` itself, so applications can locate config files in one
call. Specifically:

- **Provide a library function (or extend the derive macro):** For example, a
  function
  `ortho_config::discover_config(prefix: &str) -> OrthoResult<Figment>` could
  encapsulate the logic of finding and loading the first available config file.
  It would internally perform what the example does: check for an explicit path
  (using a convention like `<PREFIX>_CONFIG_PATH` environment variable), then
  fall back to standard directories (XDG on Unix, `AppData` on Windows, etc.),
  and finally the current directory. This leverages existing internal utilities
  – for instance, the library already has a `candidate_paths` helper for
  subcommands that enumerates
  `~/.<app>.toml`, `$XDG_CONFIG_HOME/<app>/config.toml`, `./.<app>.toml`,
  etc.([2](ortho_config/src/subcommand/paths.rs#L180-L188)).
   By extending this to also handle a user-provided path or environment
  override, the library can return a merged Figment (or config struct) in one
  step.

- **Reduce boilerplate for developers:** With this in place, the example’s
  discovery module could be replaced by a single call, e.g.
  `let figment = ortho_config::discover_config("hello_world")?;`, eliminating
  the need to manually push candidate paths and iterate through
  them([1](examples/hello_world/src/cli/discovery.rs#L14-L23))([1](examples/hello_world/src/cli/discovery.rs#L106-L115)).
   The library would ensure the search order is correct (explicit path first,
  then XDG/standard locations, then local file) and handle OS differences
  internally. This follows the “convention over configuration” goal from the
  design docs by providing sensible default behavior for config file lookup.

- **Customization via attributes:** For flexibility, the derive macro could
  allow an attribute on the root config struct to customize the config file
  name or the env var name. For example,
  `#[ortho_config(config = "hello_world", config_flag = "config")]` might
  generate a `--config` CLI flag and use `HELLO_WORLD_CONFIG_PATH` internally
  (the roadmap indicates support for renaming the config path flag is already
  planned([3](docs/roadmap.md#L114-L122))).
   This way, developers can opt-in to a custom file name or flag, but the
  underlying discovery logic remains in the library.

**Outcome:** By abstracting file discovery, an application like `hello_world`
can load configuration with one line, instead of maintaining dozens of lines of
OS-specific code. This not only reduces boilerplate but also ensures all
`ortho-config` users get a consistent discovery behavior out-of-the-box.

## 2. Generic and Composable Merging Mechanism

**Issue:** The example’s global configuration loading shows that merging
multiple sources still requires manual work. In `hello_world`, the
`load_global_config` function manually constructs a base config, then merges in
file overrides and CLI overrides using Figment
providers([4](examples/hello_world/src/cli/mod.rs#L293-L301))([4](examples/hello_world/src/cli/mod.rs#L2-L5)).
 This involves creating intermediate structs (`Overrides` and `FileLayer`) to
represent differences, then explicitly merging each into a `Figment`. While
`ortho-config` automates a lot via its derive, the need for this function
suggests the library could better handle merging without so much custom code.

**Proposal:** Introduce a **more powerful merging API or trait** in
`ortho-config` that developers can leverage to combine configuration layers in
a type-safe, declarative way:

- **`Merge` Trait for Config Structs:** Define a trait (e.g. `ConfigMerge`)
  implemented for configuration structs to combine two instances. For example,
  implementing `fn merge(&mut self, other: Self)` such that each field in
  `other` (if not default/none) overrides the value in `self`. This trait could
  be derived automatically. In practice, this would allow something like:

```rust
rustCopy code`let mut cfg = AppConfig::default();
if let Some(file_cfg) = AppConfig::from_file()? {
    cfg.merge(file_cfg);
}
cfg.merge(cli_cfg);
`
```

Under the hood, `merge` would know how to handle each field (e.g., simple
scalars get overwritten, optionals get set if `other` is `Some`, lists either
append or replace based on policy).

- **Built-in Precedence Handling:** The library’s derive could generate code to
  apply the standard precedence (CLI > Env > File > Defaults) without manual
  Figment composition. For instance, an auto-implemented `AppConfig::load()`
  might already do all merging internally. If a custom trait isn’t desirable,
  at least providing a higher-level method to merge an *overrides struct* into
  a base config would help. This is hinted in the design: the crate intended a
  single derive to “handle the entire lifecycle of parsing, layering,
  merging”([5](docs/design.md#L14-L22)).
   Realizing that vision fully means fewer ad-hoc merges in user code.

- **Array and List Merging Strategies:** As part of a more expressive merge
  mechanism, support configurable strategies for merging arrays/vectors. The
  design documents mention an `"append"` strategy for lists (where values from
  files, env, and CLI
  accumulate)([5](docs/design.md#L8-L16)).
   Implementing this would remove the need for the example’s manual logic to
  decide if CLI salutations should override or extend defaults. For instance, a
  field attribute `#[ortho_config(strategy = "append")]` could instruct the
  derive to merge vector values by concatenation instead of replacement.
  Internally, the library could extract lists from each source and combine them
  post-deserialization([5](docs/design.md#L2-L10)).
   This would greatly simplify handling of repeated parameters like the
  `salutations` vector.

- **Error Handling Improvements:** A generic merge system can also uniformly
  handle errors. In the current example, errors from merging are wrapped into a
  custom `HelloWorldError` enum. A library-provided merge would return a
  standard `OrthoError::Merge` on failure (already supported via
  `into_ortho_merge()` when using
  Figment([4](examples/hello_world/src/cli/mod.rs#L2-L5))),
   reducing the need for custom error conversion code.

**Outcome:** Developers would no longer need to write functions like
`load_global_config` to manually orchestrate merging. They could rely on either
a single call (e.g. `HelloWorldCli::load()` if extended to cover subcommands)
or use the new `Merge` trait methods to combine structures. This makes
configuration composition more *composable* – different config pieces could be
merged with each other in a clean, trait-driven manner – and less brittle,
since the logic lives in the library. The explicit merging code in the example
(creating `Overrides`, calling `figment.merge(...)` for each
layer([4](examples/hello_world/src/cli/mod.rs#L293-L301)))
 would be replaced by library calls, making the application code cleaner and
easier to follow.

## 3. Streamlined Subcommand Configuration and Overrides

**Issue:** Handling configuration for subcommands currently requires extra
boilerplate. In the example, the `greet` subcommand needed a custom step to
apply file-based overrides on top of CLI inputs. After calling
`args.load_and_merge()` for the `GreetCommand`, the code calls
`apply_greet_overrides(&mut merged)` to inject any `[cmds.greet]` values from
the config file into the final
struct([4](examples/hello_world/src/cli/mod.rs#L343-L351)).
 This was necessary because the library did not automatically apply those
overrides in the merged result – likely due to how default CLI values (like the
default `"!"` punctuation) were treated as overriding file values. In contrast,
the `take-leave` subcommand did not have special overrides in the example, but
it highlights an inconsistency in how different subcommands might need extra
code.

**Proposal:** Enhance `ortho-config` to natively support subcommand-specific
config layering and custom merge logic, so that such overrides happen
transparently:

- **Automatic Section Merging:** The library already supports defining config
  file sections for subcommands (the `load_and_merge_subcommand` function
  focuses on `[cmds.<name>]` in each
  file([6](ortho_config/src/subcommand/mod.rs#L34-L42))([6](ortho_config/src/subcommand/mod.rs#L36-L44))).
   This should be leveraged such that calling `args.load_and_merge()` on a
  subcommand *always* accounts for any file or env values specific to that
  subcommand. If there are gaps (like the `greet` punctuation default issue),
  the implementation should address them. One approach is to treat **clap
  default values as “absent” if the user didn’t override them** – e.g., the
  derive macro could internally represent optional CLI inputs as `Option`
  fields so that if a flag isn’t provided, the field stays `None` and doesn’t
  override file defaults. In the `greet` example, if `punctuation` were
  optional internally, `args.load_and_merge()` would leave it unset and thus
  the file’s `cmds.greet.punctuation` would merge in. The roadmap explicitly
  mentions distinguishing values explicitly provided on CLI vs defaults to
  avoid incorrect
  overrides([3](docs/roadmap.md#L72-L80));
   implementing that for subcommands would eliminate the need for manual
  fix-ups like `apply_greet_overrides`.

- **Custom Merge Hooks for Subcommands:** For advanced cases, allow developers
  to inject custom merging logic per subcommand. This could be done via a trait
  or an attribute. For example, a trait `SubcommandMergeHook` with a method
  `fn merge_with_defaults(&mut self, defaults: &Self)` could be implemented on
  a subcommand struct. The library would call this hook after loading defaults
  and before finalizing the subcommand config. In practice, an attribute like
  `#[ortho_config(custom_merge = "my_merge_fn")]` on the subcommand struct
  could direct the derive to call a specified function to adjust the merged
  result. This would let developers handle special cases. In most situations it
  wouldn’t be needed, but it provides an *escape hatch* for tricky merging
  scenarios that the library might not cover generically.

- **Unified Global and Subcommand Loading:** Another improvement is offering a
  one-stop API for subcommand configs. For example, if the main CLI struct has
  a `Commands` enum, `ortho-config` could provide a method to directly get the
  merged configuration for the chosen subcommand along with globals. This might
  involve enhancing the derive to generate code that detects which subcommand
  was selected and loads its config accordingly. While this is a larger design
  change, it would mean the `main.rs` logic could be simplified to something
  like:

```rust
rustCopy code`let cli = CommandLine::parse();  
let cfg = OrthoConfig::load_for_subcommand(&cli)?;  
// where cfg would contain both global and selected subcommand settings merged.
`
```

Internally, this would call the appropriate subcommand’s `load_and_merge`. This
removes duplicate code in the `match` for each subcommand, and guarantees
consistent handling (no subcommand accidentally omitting an overrides step).

**Outcome:** Subcommands would be first-class citizens in the configuration
system, with the library doing the heavy lifting of merging default values,
config file sections, environment variables, and CLI inputs for each command.
The explicit override application seen in
`apply_greet_overrides`([4](examples/hello_world/src/cli/mod.rs#L343-L351))
 could be dropped entirely – the `GreetCommand::load_and_merge()` would yield a
fully merged struct. This makes the code for running subcommands cleaner and
less error-prone (developers won’t forget to apply overrides for a new
subcommand, for example). It also aligns with the library’s goal of *reducing
friction* for common patterns: as noted in the subcommand refinements
discussion, the aim is to eliminate workarounds like manually handling a
required arg or override for a
subcommand([7](docs/subcommand-refinements.md#L12-L20))([7](docs/subcommand-refinements.md#L24-L33)).
 By implementing these improvements, `ortho-config` would handle those patterns
out-of-the-box.

By implementing these proposals – **config discovery abstraction, a generic
merge mechanism, and enhanced subcommand config support** – the `ortho-config`
library can significantly reduce boilerplate in applications. The `hello_world`
example, intended to showcase the library, highlights the areas for
improvement. Addressing them will not only simplify this example (making it a
clearer demo of best practices) but also benefit all developers using
`ortho-config` by providing a more ergonomic and powerful API. Each proposal is
focused on preserving the library’s flexibility (through traits or optional
overrides) while adopting sensible defaults that cover the majority of
use-cases without extra code. This aligns with the project’s vision of
**“dramatic reduction in boilerplate and cognitive load”** for configuration
management([5](docs/design.md#L24-L31)),
 moving closer to a truly *batteries-included* experience.
