Feature: hello_world agent context command

  Scenario: Emitting machine-readable context as JSON
    When I run the hello world example with arguments "context --json"
    Then the command succeeds
    And stdout contains a valid agent-context payload with kind "hello_world.agent_context"

  Scenario: Bare context prints a pointer to --json
    When I run the hello world example with arguments "context"
    Then the command succeeds
    And stdout contains "--json"
