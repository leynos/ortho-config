Feature: cargo-orthohelp agent-native policy check

  Scenario: destructive command without a bypass flag fails deny mode
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo orthohelp with --check-agent-native=deny
    Then the policy report on stdout contains code "destructive_bypass_missing"
    And the process exit code is 3

  Scenario: warn mode reports findings without failing
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo orthohelp with --check-agent-native=warn
    Then the policy report on stdout contains code "destructive_bypass_missing"
    And the process exit code is 0

  Scenario: the check composes with agent-context generation
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo orthohelp with format agent-context and --check-agent-native=warn
    Then the agent context file is written to the output directory
    And the policy report on stdout is valid JSON
