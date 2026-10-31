Feature: cargo-orthohelp agent-context generation

  Scenario: Generate agent-context JSON from the fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the fixture
    Then stdout contains agent-context JSON for the fixture

  Scenario: Generate agent-context JSON from the simple fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the simple fixture
    Then stdout contains agent-context JSON for the simple fixture

  Scenario: Generate nested agent-context JSON from the fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the nested fixture
    Then stdout contains nested agent-context command paths for the fixture

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
    And the command admin grant-access reports mutation effect submit
    And the command version reports interaction mode unknown

  Scenario: Independent support survives generation
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the nested fixture
    Then the command greet reports human renderer supported true
    And the command greet reports machine renderer supported true
    And the command greet reports human renderer policy tty=adapts
    And the command greet reports human renderer policy colour=auto
    And the command greet reports human renderer policy width=terminal
    And the command greet reports machine renderer policy stdout=structured
    And the command greet reports machine renderer policy colour=never
    And the command greet reports machine renderer policy width=unbounded

  Scenario: Child declarations are independent
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format agent-context for the nested fixture
    Then the undeclared nested command keeps legacy support defaults
    And the undeclared nested command makes no presentation claim
    And the command greet reports machine renderer supported true

  Scenario: Human language does not rename protocol fields
    Given a temporary output directory
    And the orthohelp cache is empty
    When I generate the nested fixture IR for two locales
    Then renderer field identifiers and policies agree across en-US and fr-FR
    And the en-US and fr-FR command summaries differ
