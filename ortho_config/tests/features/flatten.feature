Feature: Flattened argument merging
  Scenario: configuration retains flattened defaults
    Given the flattened configuration file has value "file"
    When the flattened config is loaded without CLI overrides
    Then the flattened value is "file"

  Scenario: CLI overrides flattened configuration
    Given the flattened configuration file has value "file"
    When the flattened config is loaded with CLI value "cli"
    Then the flattened value is "cli"

  Scenario: malformed flattened configuration fails
    Given a malformed flattened configuration file
    When the flattened config is loaded without CLI overrides
    Then flattening fails with a merge error
