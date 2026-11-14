Feature: Validate skill claims against an agent command contract

  Scenario: Report a command reference with an unknown flag
    Given a command contract with a nested command and effective options
    And an operation skill fixture with a valid reference and an unknown flag
    When the consumer adapter submits the extracted claims
    Then the report resolves the valid reference
    And reports the unknown flag for the referenced command
    And identifies the manifest and source ordinal

  Scenario: Check normalized example values and global options
    Given a command contract with explicit effective global options
    And deny policy without an exception for that option
    And a build workflow example with an option name and value
    When the adapter submits the normalized example
    Then the option is checked on that command
    And value presence and contents are preserved

  Scenario: Report a noncanonical option under deny policy
    Given a real command accepting the declared global option "colour-policy"
    And an example using "colour-policy" with value "always"
    And deny policy without an exception for that option
    When the consumer submits the normalized example
    Then structural validation succeeds
    And the policy finding recommends "--color"

  Scenario: A root-only option is unavailable on a child
    Given a root command with a non-global option "root-only"
    And a child whose explicit effective options omit "root-only"
    And a child example declaring and using "root-only"
    When the consumer submits it for validation
    Then an unknown-option structural finding is returned

  Scenario: Accept a prose-only skill without implying coverage
    Given a manifest with no command references or examples
    When the consumer submits it for validation
    Then no coverage error is reported
    And checked command and example counts are zero