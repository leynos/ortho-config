Feature: Cargo external-subcommand entry point

  Scenario: Cargo dispatch invocation parses the inner options
    Given a hand-built clap command named "demo" with a "--verbose" flag
    When the command is wrapped for the installed binary "cargo-demo"
    And the wrapper parses the Cargo-injected arguments "demo --verbose"
    Then parsing succeeds and the "demo" subcommand sees "--verbose"

  Scenario: Bare invocation without the injected token is rejected
    Given a hand-built clap command named "demo" with a "--verbose" flag
    When the command is wrapped for the installed binary "cargo-demo"
    And the wrapper parses the arguments "--verbose"
    Then parsing fails because the subcommand token is missing
