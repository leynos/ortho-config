Feature: cargo-orthohelp agent-native policy reports

  Scenario: Emit an empty warning policy report
    Given a temporary output directory
    When I run cargo-orthohelp policy check in warn mode for the simple fixture
    Then the policy report has warn mode and no findings

  Scenario: Emit a warning policy finding
    Given a temporary output directory
    When I run cargo-orthohelp policy check in warn mode for the fixture
    Then the policy report has one warning finding

  Scenario: Emit a deny policy finding and fail validation
    Given a temporary output directory
    When I run cargo-orthohelp policy check in deny mode for the fixture
    Then the policy report has one deny finding and a validation failure

  Scenario: Suppress policy evaluation in off mode
    Given a temporary output directory
    When I run cargo-orthohelp policy check in off mode for the fixture
    Then the policy report has off mode and no findings
