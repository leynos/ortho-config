Feature: Ignore patterns
  Scenario: merge ignore patterns from env and CLI
    Given the environment variable DDLINT_IGNORE_PATTERNS is ".git/,build/"
    When the config is loaded with CLI ignore "target/"
    Then the ignore patterns are ".git/,build/,target/"

  Scenario: ignore patterns handle whitespace
    Given the environment variable DDLINT_IGNORE_PATTERNS is " .git/ , build/ "
    When the config is loaded with CLI ignore " target/ "
    Then the ignore patterns are ".git/,build/,target/"

  Scenario: ignore patterns with duplicates
    Given the environment variable DDLINT_IGNORE_PATTERNS is ".git/,.git/"
    When the config is loaded with CLI ignore ".git/"
    Then the ignore patterns are ".git/,.git/,.git/"

  Scenario: ignore patterns with empty environment variable
    Given the environment variable DDLINT_IGNORE_PATTERNS is ""
    When the config is loaded with CLI ignore "target/"
    Then the ignore patterns are "target/"

  Scenario: ignore patterns with no CLI argument
    Given the environment variable DDLINT_IGNORE_PATTERNS is ".git/,build/"
    When the config is loaded with CLI ignore ""
    Then the ignore patterns are ".git/,build/"

