//! Binds the `ortho_config` behavioural feature files to the step registry.

use rstest_bdd_macros::scenarios;

scenarios!("tests/features/cli_precedence.feature");
scenarios!("tests/features/collection_merge.feature");
scenarios!("tests/features/config_path.feature");
scenarios!("tests/features/csv_env.feature");
scenarios!("tests/features/error_aggregation.feature");
scenarios!("tests/features/extends.feature");
scenarios!("tests/features/flatten.feature");
scenarios!("tests/features/ignore_patterns.feature");
scenarios!("tests/features/subcommand.feature");
scenarios!("tests/features/localizer.feature");
