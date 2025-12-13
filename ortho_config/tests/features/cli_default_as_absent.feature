Feature: CLI default as absent
  The `cli_default_as_absent` attribute allows non-Option fields with clap
  defaults to be treated as absent when the user did not override them on the
  CLI. This enables file and environment configuration to take precedence over
  clap defaults while still honouring explicit CLI overrides.

  Precedence (lowest to highest): struct defaults < file < environment < CLI.

  Scenario: file value overrides clap default
    Given a clap default punctuation "!"
    And a file punctuation "?"
    When the subcommand configuration is merged
    Then the resolved punctuation is "?"

  Scenario: environment overrides clap default
    Given a clap default punctuation "!"
    And an environment punctuation "..."
    When the subcommand configuration is merged
    Then the resolved punctuation is "..."

  Scenario: environment overrides file
    Given a clap default punctuation "!"
    And a file punctuation "?"
    And an environment punctuation "..."
    When the subcommand configuration is merged
    Then the resolved punctuation is "..."

  Scenario: explicit CLI overrides all
    Given a clap default punctuation "!"
    And a file punctuation "?"
    And an environment punctuation "..."
    And an explicit CLI punctuation "!!!"
    When the subcommand configuration is merged
    Then the resolved punctuation is "!!!"

  Scenario: clap default excluded from extraction
    Given a clap default punctuation "!"
    When CLI values are extracted
    Then punctuation is absent from extracted values

  Scenario: explicit CLI included in extraction
    Given an explicit CLI punctuation "!!!"
    When CLI values are extracted
    Then punctuation is present in extracted values
