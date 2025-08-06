Feature: CLI precedence
  Scenario: CLI overrides environment and file values
    Given the configuration file has rules "file"
    And the environment variable DDLINT_RULES is "env"
    When the config is loaded with CLI rules "cli"
    Then the loaded rules are "cli"
