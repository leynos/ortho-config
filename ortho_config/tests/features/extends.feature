Feature: Configuration inheritance
  Scenario: load config from base file
    Given a configuration file extending a base file
    When the extended configuration is loaded
    Then the rules are "child"

  Scenario: cyclic inheritance error
    Given a configuration file with cyclic inheritance
    When the cyclic configuration is loaded
    Then an error occurs

  Scenario: missing base file error
    Given a configuration file extending a missing base file
    When the configuration with missing base is loaded
    Then an error occurs

  Scenario: multi-level inheritance
    Given a configuration file extending a parent file that extends a grandparent file
    When the multi-level configuration is loaded
    Then the inherited rules are "child"
