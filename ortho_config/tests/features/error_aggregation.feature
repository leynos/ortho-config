Feature: Error aggregation
  Scenario: Collects errors from all sources
    Given an invalid configuration file
    And the environment variable DDLINT_PORT is "notanumber"
    When the config is loaded with an invalid CLI argument
    Then CLI, file and environment errors are returned

  Scenario: Reports a single CLI error
    When the config is loaded with an invalid CLI argument
    Then a CLI parsing error is returned
