Feature: Collection merge strategies
  Scenario: Replace map semantics drop lower precedence entries
    Given the dynamic rules config enables "file" via the configuration file
    And the environment defines dynamic rule "cli" as enabled
    When the configuration is loaded with replace map semantics
    Then only the dynamic rule "cli" is enabled
