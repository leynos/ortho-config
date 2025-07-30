Feature: Configuration inheritance
  Scenario: load config from base file
    Given a configuration file extending a base file
    When the extended configuration is loaded
    Then the rules are "child"
