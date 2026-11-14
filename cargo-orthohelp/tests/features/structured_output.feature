Feature: Declared structured output capability

  Scenario: A data command has no JSON declaration
    Given a command with stdout purpose "data" and undeclared JSON support
    When the output policy is checked in "deny" mode
    Then exactly one "json_support_missing" finding has severity "deny"

  Scenario: A data command supports JSON
    Given a command with stdout purpose "data" and JSON support true
    When the output policy is checked in "warn" mode
    Then the output policy report has no findings

  Scenario: Enforcement is disabled
    Given a command with stdout purpose "data" and JSON support false
    When the output policy is checked in "off" mode
    Then the output policy report has no findings

  Scenario: Unknown output is not inferred from a verb
    Given a command named "get" with no output declaration
    When the output policy is checked in "deny" mode
    Then the output policy report has no findings