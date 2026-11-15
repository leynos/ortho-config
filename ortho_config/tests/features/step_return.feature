Feature: Aliased step return values through the scenario runner
  The workspace declares every step as a fallible alias, so the generated
  scenario runner must treat an aliased `Result` as fallible end to end
  rather than only inside the step wrapper.

  Scenario: A failing aliased step fails the scenario
    Given a step return alias reports failure
    Then the scenario must have stopped

  Scenario: A failing boxed-error aliased step fails the scenario
    Given a boxed-error step alias reports failure
    Then the scenario must have stopped

  Scenario: An aliased payload reaches a later step
    Given a step return alias reports a level of 7
    Then the injected level is 7

  Scenario: A boxed-error aliased payload reaches a later step
    Given a boxed-error step alias reports a level of 3
    Then the injected level is 3

  Scenario: An aliased unit step produces no override
    Given a step return alias succeeds with no payload
    Then the injected level is 5
