//! Fixture implementation constructing a localized nested command tree.

macro_rules! define_nested_fixture {
    () => {
        /// Builds a localized nested command tree matching the `ortho_config`
        /// behavioural fixture.
        #[must_use]
        pub fn nested_doc() -> LocalizedDocMetadata {
            let mut root = command("nested-app", "Nested fixture command tree.");
            root.bin_name = Some("fixture".to_owned());
            root.fields.push(text_field(TextFieldSpec {
                name: "global",
                help: "Global configuration value.",
                long: "global",
                env: Some("NESTED_APP_GLOBAL"),
                default: None,
                required: true,
            }));
            root.windows = Some(WindowsMetadata {
                module_name: Some("Nested".to_owned()),
                export_aliases: Vec::new(),
                include_common_parameters: true,
                split_subcommands_into_functions: false,
                help_info_uri: None,
            });
            root.subcommands = vec![greet_command(), version_command(), admin_command()];
            root
        }

        fn greet_command() -> LocalizedDocMetadata {
            let mut metadata = command("greet", "Greets a named recipient.");
            metadata.sections.examples.push(LocalizedExample {
                title: Some("Greet Ada".to_owned()),
                code: "fixture greet --recipient Ada".to_owned(),
                body: Some("Prints a greeting for Ada.".to_owned()),
            });
            metadata.fields.push(flag_field(
                "excited",
                "Adds an exclamation mark to the greeting.",
                "excited",
            ));
            metadata.fields.push(text_field(TextFieldSpec {
                name: "recipient",
                help: "Recipient to greet.",
                long: "recipient",
                env: Some("NESTED_APP_RECIPIENT"),
                default: Some("World"),
                required: false,
            }));
            metadata
        }

        fn version_command() -> LocalizedDocMetadata {
            command("version", "Prints version information.")
        }

        fn admin_command() -> LocalizedDocMetadata {
            let mut metadata = command("admin", "Administers fixture state.");
            metadata.fields.push(text_field(TextFieldSpec {
                name: "scope",
                help: "Scope to administer.",
                long: "scope",
                env: Some("NESTED_APP_SCOPE"),
                default: Some("local"),
                required: false,
            }));
            metadata.windows = Some(WindowsMetadata {
                module_name: Some("NestedAdmin".to_owned()),
                export_aliases: Vec::new(),
                include_common_parameters: false,
                split_subcommands_into_functions: true,
                help_info_uri: None,
            });
            metadata.subcommands = vec![audit_command(), grant_command()];
            metadata
        }

        fn audit_command() -> LocalizedDocMetadata {
            let mut metadata = command("audit", "Audits fixture state.");
            metadata.fields.push(flag_field(
                "dry_run",
                "Reports intended audit actions.",
                "dry-run",
            ));
            metadata
        }

        fn grant_command() -> LocalizedDocMetadata {
            let mut metadata = command("grant-access", "Grants access to a principal.");
            metadata.fields.push(text_field(TextFieldSpec {
                name: "principal",
                help: "Principal receiving access.",
                long: "principal",
                env: None,
                default: None,
                required: true,
            }));
            metadata
        }

        fn command(app_name: &str, about: &str) -> LocalizedDocMetadata {
            LocalizedDocMetadata {
                ir_version: "1.1".to_owned(),
                locale: "en-US".to_owned(),
                app_name: app_name.to_owned(),
                bin_name: None,
                about: about.to_owned(),
                synopsis: None,
                sections: LocalizedSectionsMetadata {
                    headings: headings(),
                    discovery: None,
                    precedence: None,
                    examples: Vec::new(),
                    links: Vec::new(),
                    notes: Vec::new(),
                },
                fields: Vec::new(),
                subcommands: Vec::new(),
                windows: None,
            }
        }

        fn headings() -> LocalizedHeadings {
            LocalizedHeadings {
                name: "NAME".to_owned(),
                synopsis: "SYNOPSIS".to_owned(),
                description: "DESCRIPTION".to_owned(),
                options: "OPTIONS".to_owned(),
                environment: "ENVIRONMENT".to_owned(),
                files: "FILES".to_owned(),
                precedence: "PRECEDENCE".to_owned(),
                exit_status: "EXIT STATUS".to_owned(),
                examples: "EXAMPLES".to_owned(),
                see_also: "SEE ALSO".to_owned(),
                commands: "COMMANDS".to_owned(),
            }
        }

        fn flag_field(name: &str, help: &str, long: &str) -> LocalizedFieldMetadata {
            LocalizedFieldMetadata {
                name: name.to_owned(),
                help: help.to_owned(),
                long_help: None,
                value: Some(ValueType::Bool),
                default: None,
                required: false,
                deprecated: None,
                cli: Some(CliMetadata {
                    long: Some(long.to_owned()),
                    short: None,
                    value_name: None,
                    multiple: false,
                    takes_value: false,
                    possible_values: Vec::new(),
                    hide_in_help: false,
                }),
                env: None,
                file: None,
                examples: Vec::new(),
                links: Vec::new(),
                notes: Vec::new(),
            }
        }

        #[derive(Clone, Copy)]
        struct TextFieldSpec<'a> {
            name: &'a str,
            help: &'a str,
            long: &'a str,
            env: Option<&'a str>,
            default: Option<&'a str>,
            required: bool,
        }

        fn text_field(spec: TextFieldSpec<'_>) -> LocalizedFieldMetadata {
            LocalizedFieldMetadata {
                name: spec.name.to_owned(),
                help: spec.help.to_owned(),
                long_help: None,
                value: Some(ValueType::String),
                default: spec.default.map(|display| DefaultValue {
                    display: display.to_owned(),
                }),
                required: spec.required,
                deprecated: None,
                cli: Some(CliMetadata {
                    long: Some(spec.long.to_owned()),
                    short: None,
                    value_name: Some(spec.name.to_ascii_uppercase()),
                    multiple: false,
                    takes_value: true,
                    possible_values: Vec::new(),
                    hide_in_help: false,
                }),
                env: spec.env.map(|var_name| EnvMetadata {
                    var_name: var_name.to_owned(),
                }),
                file: None,
                examples: Vec::new(),
                links: Vec::new(),
                notes: Vec::new(),
            }
        }
    };
}

pub(crate) use define_nested_fixture;
