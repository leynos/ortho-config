Feature: Profile selection and precedence

  Scenario: Selected profile overlays file values
    Given a config file with key "retries" set to "3"
    And the same file defines profile "ci" with "retries" set to "7"
    When the CLI loads with "--profile ci"
    Then the merged value of "retries" is "7"

  Scenario: Environment beats the selected profile
    Given a config file defining profile "ci" with "retries" set to "7"
    And the environment sets the "retries" key to "9"
    When the CLI loads with "--profile ci"
    Then the merged value of "retries" is "9"

  Scenario: An explicit flag equal to the default beats the profile
    Given a struct default of "3" for "retries"
    And a config file defining profile "ci" with "retries" set to "7"
    When the CLI loads with "--profile ci --retries 3"
    Then the merged value of "retries" is "3"

  Scenario: The profile flag beats the selector environment variable
    Given a config file defining profiles "ci" and "local"
    And the selector environment variable names profile "local"
    When the CLI loads with "--profile ci"
    Then the selected profile is "ci" with source "flag"

  Scenario: Selecting an unknown profile fails with the available names
    Given a config file defining profiles "ci" and "local"
    When the CLI loads with "--profile staging"
    Then loading fails naming "staging" from source "flag"
    And the error lists available profiles "ci" and "local"

  Scenario: An env-selected profile with no config files fails clearly
    Given no configuration files are discoverable
    And the selector environment variable names profile "ci"
    When the CLI loads
    Then loading fails naming "ci" from the selector environment variable
    And the error states that no configuration files were found

  Scenario: A profile table must not configure subcommands
    Given a config file defining profile "ci" containing a "cmds" table
    When the CLI loads with "--profile ci"
    Then loading fails identifying the forbidden "cmds" key in "ci"

  Scenario: An explicit flag beats the environment for the merged value
    Given a config file defining profile "ci" with "retries" set to "7"
    And the environment sets the "retries" key to "9"
    When the CLI loads with "--profile ci --retries 11"
    Then the merged value of "retries" is "11"

  Scenario: The profile selector never leaks into composed layers
    Given a config file with key "retries" set to "3"
    And the same file defines profile "ci" with "retries" set to "7"
    And the selector environment variable names profile "ci"
    When the CLI loads
    Then the merged value of "retries" is "7"
    And no composed layer contains a "profile" key