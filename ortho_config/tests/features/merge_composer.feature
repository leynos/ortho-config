Feature: Merge composer builder assembles layers

  Scenario: Composing layers captures file, environment, and CLI sources
    Given the configuration file has rules "file-rule"
    And the environment variable DDLINT_RULES is "env-rule"
    When the rule layers are composed with CLI rules "cli-rule"
    Then the composed layer order is defaults, file, environment, cli
    And the merged rules resolve to "file-rule,env-rule,cli-rule"
