Feature: Subcommand defaults
  Scenario: CLI fills missing required field
    Given a CLI reference "https://example.com/pr/1"
    When the subcommand configuration is loaded without defaults
    Then the merged reference is "https://example.com/pr/1"
