//! Binds the hello_world behavioural feature file to rstest-bdd scenarios.

use crate::behaviour::harness::Harness;
use rstest_bdd_macros::scenario;

macro_rules! hello_world_scenario {
    ($fn_name:ident, $scenario_name:literal) => {
        #[scenario(
            path = "tests/features/global_parameters.feature",
            name = $scenario_name,
            tags = "not @requires.yaml"
        )]
        fn $fn_name(hello_world_harness: Harness) {
            let _ = hello_world_harness;
        }
    };
    ($fn_name:ident, $scenario_name:literal, yaml) => {
        #[cfg(feature = "yaml")]
        #[scenario(
            path = "tests/features/global_parameters.feature",
            name = $scenario_name,
            tags = "@requires.yaml"
        )]
        fn $fn_name(hello_world_harness: Harness) {
            let _ = hello_world_harness;
        }
    };
}

hello_world_scenario!(
    using_defaults_produces_a_friendly_greeting,
    "Using defaults produces a friendly greeting"
);
hello_world_scenario!(cli_help_exits_successfully, "CLI help exits successfully");
hello_world_scenario!(cli_version_exits_successfully, "CLI version exits successfully");
hello_world_scenario!(
    excited_delivery_shouts_the_greeting,
    "Excited delivery shouts the greeting"
);
hello_world_scenario!(
    quiet_delivery_softens_the_message,
    "Quiet delivery softens the message"
);
hello_world_scenario!(
    conflicting_delivery_modes_are_rejected,
    "Conflicting delivery modes are rejected"
);
hello_world_scenario!(custom_salutations_are_honoured, "Custom salutations are honoured");
hello_world_scenario!(
    printing_a_preamble_before_greeting,
    "Printing a preamble before greeting"
);
hello_world_scenario!(
    taking_leave_summarises_the_farewell,
    "Taking leave summarises the farewell"
);
hello_world_scenario!(
    taking_leave_customises_the_greeting,
    "Taking leave customises the greeting"
);
hello_world_scenario!(
    cli_overrides_environment_overrides_configuration_files,
    "CLI overrides environment overrides configuration files"
);
hello_world_scenario!(
    explicit_config_path_overrides_discovery_order,
    "Explicit config path overrides discovery order"
);
hello_world_scenario!(
    cli_config_flag_selects_a_custom_file,
    "CLI config flag selects a custom file"
);
hello_world_scenario!(
    yaml_scalars_remain_strings,
    "YAML 1.2 scalars remain strings",
    yaml
);
hello_world_scenario!(
    duplicate_yaml_keys_are_rejected,
    "Duplicate YAML keys are rejected",
    yaml
);
hello_world_scenario!(
    canonical_yaml_booleans_remain_booleans,
    "Canonical YAML booleans remain booleans",
    yaml
);
hello_world_scenario!(
    xdg_config_home_provides_configuration,
    "XDG config home provides configuration"
);
hello_world_scenario!(
    sample_configs_drive_demo_scripts,
    "Sample configuration files drive the demo scripts"
);
hello_world_scenario!(
    missing_sample_configuration_falls_back_to_defaults,
    "Missing sample configuration falls back to defaults"
);
hello_world_scenario!(
    declarative_merging_composes_layered_overrides,
    "Declarative merging composes layered overrides"
);
hello_world_scenario!(
    declarative_merging_accumulates_repeated_append_contributions,
    "Declarative merging accumulates repeated append contributions"
);
