Feature: Global parameters govern greetings
  The hello_world example demonstrates global flags, switches, and arrays.

  Scenario: Using defaults produces a friendly greeting
    When I run the hello world example with arguments "greet"
    Then the command succeeds
    And stdout contains "Hello, World!"

  Scenario: CLI help exits successfully
    When I run the hello world example with arguments "--help"
    Then the command succeeds
    And stdout contains "Friendly greeting demo showcasing localized help"

  Scenario: CLI version exits successfully
    When I run the hello world example with arguments "--version"
    Then the command succeeds
    And stdout contains the hello world version

  Scenario: Excited delivery shouts the greeting
    When I run the hello world example with arguments "--is-excited greet"
    Then the command succeeds
    And stdout contains "HELLO, WORLD!"

  Scenario: Quiet delivery softens the message
    When I run the hello world example with arguments "--is-quiet greet"
    Then the command succeeds
    And stdout contains "Hello, World..."

  Scenario: Conflicting delivery modes are rejected
    When I run the hello world example with arguments "--is-excited --is-quiet greet"
    Then the command fails
    And stderr contains "cannot combine --is-excited with --is-quiet"

  Scenario: Custom salutations are honoured
    When I run the hello world example with arguments "-s Hi -s there -r team greet"
    Then the command succeeds
    And stdout contains "Hi there, team!"

  Scenario: Printing a preamble before greeting
    When I run the hello world example with arguments "greet --preamble 'Good morning'"
    Then the command succeeds
    And stdout contains "Good morning"

  Scenario: Taking leave summarises the farewell
    When I run the hello world example with arguments "--is-excited take-leave --gift biscuits --remind-in 15 --channel email --wave"
    Then the command succeeds
    And stdout contains "HELLO, WORLD!"
    And stdout contains "leaves biscuits"
    And stdout contains "follows up with an email"

  Scenario: Taking leave customises the greeting
    When I run the hello world example with arguments "take-leave --preamble 'Until next time' --punctuation ?"
    Then the command succeeds
    And stdout contains "Until next time"
    And stdout contains "Hello, World?"

  Scenario: CLI overrides environment overrides configuration files
    Given the hello world config file contains:
      """
      recipient = "File"
      salutations = ["File hello"]
      """
    And the environment contains "HELLO_WORLD_RECIPIENT" = "Env"
    And the environment contains "HELLO_WORLD_SALUTATIONS" = "EnvOne,EnvTwo"
    When I run the hello world example with arguments "-r Cli greet"
    Then the command succeeds
    And stdout contains "EnvOne EnvTwo, Cli!"

  Scenario: CLI salutations replace discovered layers
    Given the hello world config file contains:
      """
      recipient = "Layered"
      salutations = ["From file"]
      """
    And the environment contains "HELLO_WORLD_SALUTATIONS" = "From env"
    When I run the hello world example with arguments "-s FromCli greet"
    Then the command succeeds
    And stdout contains "FromCli, Layered!"

  Scenario: Explicit config path overrides discovery order
    Given the file "custom.toml" contains:
      """
      recipient = "Explicit path"
      """
    And the environment contains "HELLO_WORLD_CONFIG_PATH" = "custom.toml"
    When I run the hello world example with arguments "greet"
    Then the command succeeds
    And stdout contains "Explicit path"

  Scenario: CLI config flag selects a custom file
    Given the file "cli.toml" contains:
      """
      recipient = "CLI config"
      """
    When I run the hello world example with arguments "--config cli.toml greet"
    Then the command succeeds
    And stdout contains "CLI config"

  @requires.yaml
  Scenario: YAML 1.2 scalars remain strings
    Given the file "semantic.yaml" contains:
      """
      recipient: yes
      """
    When I run the hello world example with arguments "--config semantic.yaml greet"
    Then the command succeeds
    And stdout contains "Hello, yes!"

  @requires.yaml
  Scenario: Duplicate YAML keys are rejected
    Given the file "duplicate.yaml" contains:
      """
      recipient: first
      recipient: second
      """
    When I run the hello world example with arguments "--config duplicate.yaml greet"
    Then the command fails
    And stderr contains "duplicate mapping key"

  @requires.yaml
  Scenario: Canonical YAML booleans remain booleans
    Given the file "canonical_bools.yaml" contains:
      """
      recipient: world
      is_excited: true
      is_quiet: false
      """
    When I run the hello world example with arguments "--config canonical_bools.yaml greet"
    Then the command succeeds
    And stdout contains "HELLO, WORLD!"

  Scenario: XDG config home provides configuration
    Given the XDG config home contains:
      """
      recipient = "XDG home"
      """
    When I run the hello world example with arguments "greet"
    Then the command succeeds
    And stdout contains "XDG home"

  Scenario: Sample configuration files drive the demo scripts
    Given I start from the sample hello world config "overrides.toml"
    When I run the hello world example with arguments "greet"
    Then the command succeeds
    And stdout contains "Layered hello"
    And stdout contains "HEY CONFIG FRIENDS, EXCITED CREW!!!"

  Scenario: Missing sample configuration falls back to defaults
    Given I start from a missing or invalid sample config "nonexistent.toml"
    When I run the hello world example with arguments "greet"
    Then the command succeeds
    And stdout contains "Hello, World!"

  Scenario: Declarative merging composes layered overrides
      Given I compose hello world globals from declarative layers:
        """
        [
          {"provenance": "defaults", "value": {"recipient": "Defaults", "salutations": ["Hi"], "is_excited": false, "is_quiet": false}},
          {"provenance": "environment", "value": {"salutations": ["Env"]}},
          {"provenance": "cli", "value": {"recipient": "Cli"}}
        ]
        """
    Then the declarative globals recipient is "Cli"
    And the declarative globals salutations are:
      """
      Hi
      Env
      """

  Scenario: Declarative merging accumulates repeated append contributions
      Given I compose hello world globals from declarative layers:
        """
        [
          {"provenance": "defaults", "value": {"recipient": "Defaults", "salutations": ["Defaults"], "is_excited": false, "is_quiet": false}},
          {"provenance": "environment", "value": {"salutations": ["EnvOne"]}},
          {"provenance": "environment", "value": {"salutations": ["EnvTwo"]}},
          {"provenance": "cli", "value": {"salutations": ["CliOne", "CliTwo"]}}
        ]
        """
      Then the declarative globals salutations are:
        """
        Defaults
        EnvOne
        EnvTwo
        CliOne
        CliTwo
        """
