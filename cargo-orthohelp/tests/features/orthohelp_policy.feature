Feature: Agent-native policy check

  Scenario: Warn mode reports findings without failing
    Given the policy warn fixture package
    When cargo orthohelp runs with --check-agent-native
    Then the command succeeds
    And the policy report lists one warning with code redundant_exception
    And the policy report lists the configured exceptions
    And the policy report lists the canonical vocabulary

  Scenario: Deny mode fails on deny findings
    Given the policy deny fixture package
    When cargo orthohelp runs with --check-agent-native
    Then the command fails with a policy violation
    And the policy report summary counts one deny finding

  Scenario: Off mode suppresses checking
    Given a fixture package with no policy table
    When cargo orthohelp runs with --check-agent-native
    Then the command succeeds
    And the policy report records mode off and no findings
    And standard error notes that nothing was checked

  Scenario: Explicit off mode suppresses findings from a configured table
    Given the policy off fixture package
    When cargo orthohelp runs with --check-agent-native
    Then the command succeeds
    And the policy report records mode off and no findings
    And standard error notes that nothing was checked

  Scenario: Command-line mode override wins for the report
    Given the policy warn fixture package
    When cargo orthohelp runs with --check-agent-native --policy-mode deny
    Then the policy report records mode deny
