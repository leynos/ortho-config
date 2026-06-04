Feature: cargo-orthohelp agent-context generation

  Scenario: Generate agent-context JSON from the fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the fixture
    Then the output contains agent-context JSON for the fixture
