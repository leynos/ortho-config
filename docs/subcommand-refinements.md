# Subcommand refinements

## Current Design and Developer Experience

The existing implementation of subcommand configuration in `ortho-config` is
powerful, providing a layered approach to configuration from files, environment
variables, and command-line arguments. However, as demonstrated by the `vk`
source code, there is room for improvement in the developer experience,
particularly in the ergonomics of merging partially-defined configurations with
required command-line arguments.

In `vk`, the `main.rs` file contains a `load_with_reference_fallback` function.
This function is a workaround to handle the case where a required command-line
argument (`reference`) is not present in the configuration files or environment
variables. It attempts to load and merge the configuration, and if that fails
with a `MissingField` error for the `reference` field, it falls back to using
just the parsed command-line arguments.

This workaround highlights a point of friction in the `ortho-config` API.
Developers are required to write boilerplate code to handle a common use case:
a subcommand with required arguments that should not be expected to exist in
the configuration files.

## Proposed Improvements to `OrthoConfig`

I propose a refinement of the `OrthoConfig` trait and its derive macro to
natively support this pattern, thereby eliminating the need for workarounds
like `load_with_reference_fallback`.

### 1. A more intuitive `load_and_merge` Method

The primary change would be to the behaviour of the `load_and_merge` method
(and its `_for` variant). Instead of requiring all fields to be present in the
configuration sources, it should gracefully handle missing fields that are
already present in the `self` (the `clap`-parsed struct).

The new process for `load_and_merge` would be:

1. **Load Defaults**: Load configuration from files and environment variables
   into a `figment` instance. This step would not fail if fields are missing.

2. **Merge CLI Arguments**: Merge the `self` struct (containing the parsed CLI
   arguments) over the top of the loaded defaults.

3. **Final Extraction**: Extract the final, merged configuration. This step
   will now succeed because any fields required by `clap` will be present in
   the `self` struct.

This change would make the `load_and_merge` method more intuitive, as it would
correctly reflect the desired precedence (CLI &gt; Env &gt; File &gt; Defaults)
without requiring all values to be defined in lower-precedence layers.

### 2. Deprecating `load_subcommand_config`

The `load_subcommand_config` and `load_subcommand_config_for` functions, which
only load defaults, become less useful with the improved `load_and_merge`. To
simplify the API, these could be deprecated and eventually removed, guiding
users towards the more comprehensive `load_and_merge` as the single,
recommended way to handle subcommand configuration.

## How this Simplifies `vk`

With these proposed changes to `ortho-config`, the `vk` application's `main`
function could be significantly simplified. The `load_with_reference_fallback`
function would no longer be necessary. The `main` function would look like this:

```rust
// Simplified vk/src/main.rs

#[tokio::main]
async fn main() -> Result<(), VkError> {
    let cli = Cli::parse();
    let mut global = GlobalArgs::load_from_iter(std::env::args_os().take(1))?;
    global.merge(cli.global);

    match cli.command {
        Commands::Pr(pr_cli) => {
            // A single, clean call to load and merge configuration.
            let args = pr_cli.load_and_merge()?;
            run_pr(args, global.repo.as_deref()).await
        }
        Commands::Issue(issue_cli) => {
            let args = issue_cli.load_and_merge()?;
            run_issue(args, global.repo.as_deref()).await
        }
    }
}
```

This revised code is more readable, more concise, and more directly expresses
the developer's intent. The developer effort is reduced, and the API is more
intuitive.

## Conclusion

By refining the behaviour of `load_and_merge` to better accommodate
partially-defined configurations, `ortho-config` can provide a more ergonomic
and powerful developer experience for configuring subcommands. This change
aligns with the library's goal of reducing boilerplate and simplifying
configuration management, as evidenced by the significant simplification it
would bring to the `vk` codebase.
