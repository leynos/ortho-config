Feature: Comma-separated environment lists
  Scenario: parse environment variable into list
    Given the environment variable DDLINT_RULES is "A,B,C"
    When the configuration is loaded
    Then the rules are "A,B,C"
