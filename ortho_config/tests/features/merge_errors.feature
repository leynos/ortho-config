Feature: Merge errors surface consistently
  Scenario: Merge failures use the shared merge error surface
    Given an invalid CLI merge layer for rules
    When the rules configuration is merged
    Then a merge error is returned
