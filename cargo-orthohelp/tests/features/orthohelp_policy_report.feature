Feature: cargo-orthohelp agent-native policy reports

  Scenario: Emit an empty warning policy report
    Given a fixture package with no policy table
    When cargo orthohelp runs with --check-agent-native --policy-mode warn
    Then stdout policy report has warn mode and no findings

  Scenario: Emit a warning policy finding
    Given the policy warn fixture package
    When cargo orthohelp runs with --check-agent-native --policy-mode warn
    Then stdout contains a policy report with one warning finding

  Scenario: Emit an empty deny policy report
    Given a fixture package with no policy table
    When cargo orthohelp runs with --check-agent-native --policy-mode deny
    Then stdout policy report has deny mode and no findings

  Scenario: Emit a deny policy finding and fail validation
    Given the policy deny fixture package
    When cargo orthohelp runs with --check-agent-native --policy-mode deny
    Then stdout contains a policy report with one deny finding and a validation failure

  Scenario: Suppress policy evaluation in off mode
    Given the policy off fixture package
    When cargo orthohelp runs with --check-agent-native --policy-mode off
    Then stdout policy report has off mode and no findings
