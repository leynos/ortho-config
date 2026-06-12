Feature: Localised CLI help
  Scenario: Japanese locale localises the command tree
    Given the user's locale is "ja"
    When the user renders the hello-world long help
    Then the about line contains the Japanese greeting copy
    And the greet subcommand help contains the Japanese greeting copy

  Scenario: English locale localises the command tree
    Given the user's locale is "en-US"
    When the user renders the hello-world long help
    Then the about line contains the English localised copy
    And the greet subcommand help contains the English localised copy
