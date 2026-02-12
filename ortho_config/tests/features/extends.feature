Feature: Configuration inheritance
  Scenario: load config from base file
    Given a configuration file extending a base file
    When the extended configuration is loaded
    Then the effective rules are "base","child"

  Scenario: cyclic inheritance error
    Given a configuration file with cyclic inheritance
    When the cyclic configuration is loaded
    Then an error occurs

  Scenario: missing base file error
    Given a configuration file extending a missing base file
    When the configuration with missing base is loaded
    Then an error occurs

  Scenario: non-string extends value error
    Given a configuration file with a non-string extends value
    When the non-string extends configuration is loaded
    Then an error occurs

  Scenario: multi-level inheritance
    Given a configuration file extending a parent file that extends a grandparent file
    When the multi-level configuration is loaded
    Then the inherited rules are "grandparent","parent","child"

  Scenario: replacement strategy on annotated fields
    Given a configuration file extending a base file with replace strategy on rules
    When the replace-strategy configuration is loaded
    Then the effective rules are "child"
