Feature: Subcommand defaults
  Scenario: CLI fills missing required field
    Given a CLI reference "https://example.com/pr/1"
    When the subcommand configuration is loaded without defaults
    Then the merged reference is "https://example.com/pr/1"

  Scenario: missing CLI value errors
    Given no CLI reference
    When the subcommand configuration is loaded without defaults
    Then the subcommand load fails

  Scenario: environment provides reference
    Given no CLI reference
    And an environment reference "https://example.com/env"
    When the subcommand configuration is loaded without defaults
    Then the merged reference is "https://example.com/env"

  Scenario: CLI overrides configuration file
    Given a CLI reference "https://example.com/cli"
    And a configuration reference "https://example.com/file"
    When the subcommand configuration is loaded without defaults
    Then the merged reference is "https://example.com/cli"
