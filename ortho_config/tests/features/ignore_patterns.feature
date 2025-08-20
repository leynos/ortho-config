Feature: Ignore patterns
  Scenario: merge ignore patterns from env and CLI
    Given the environment variable DDLINT_IGNORE_PATTERNS is ".git/,build/"
    When the config is loaded with CLI ignore "target/"
    Then the ignore patterns are ".git/,build/,target/"
