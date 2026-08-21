Feature: cargo-orthohelp agent-context generation

  Scenario: Generate agent-context JSON from the fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the fixture
    Then the output contains agent-context JSON for the fixture

  Scenario: Generate nested agent-context JSON from the fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the nested fixture
    Then the output contains nested agent-context command paths for the fixture

  Scenario: agent context reports declared behaviour metadata
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the nested fixture
    Then the command admin purge reports interaction mode interactive
    And the command admin purge reports mutation effect delete
    And the command admin purge reports bypass flag --force
    And the command admin prune reports mutation effect delete
    And the command greet reports interaction mode non_interactive
    And the command greet reports mutation effect read_only
    And the command version reports interaction mode unknown
