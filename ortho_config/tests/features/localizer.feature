Feature: Localizer trait
  Localization helpers must offer predictable fallback behaviour so CLI
  surfaces can opt into translations incrementally.

  Scenario: No-op localizer falls back to defaults
    Given a noop localizer
    When I request id cli.about with fallback Default CLI about
    Then the localized text is Default CLI about

  Scenario: Subject-aware localizer formats Fluent arguments
    Given a subject-aware localizer
    When I request id cli.about for subject Ada Lovelace
    Then the localized text is Hola, Ada Lovelace! (cli.about)

  Scenario: Fluent localizer prefers consumer catalogue
    Given a fluent localizer with consumer overrides
    When I request id cli.about with fallback Default CLI about
    Then the localized text is Localised about from consumer

  Scenario: Fluent localizer logs formatting errors and falls back
    Given a fluent localizer with a mismatched template
    When I request id cli.usage for binary demo-cli
    Then the localized text is Usage: demo-cli [OPTIONS] <COMMAND>
    And a localisation formatting error is recorded

  Scenario: Clap errors localize when translations exist
    Given a clap-aware localizer
    And a clap error for a missing argument
    When I localize the clap error
    Then the localized text contains clap-error-missing-argument
    And the localized text includes the clap argument label

  Scenario: Clap errors fall back without translations
    Given a noop localizer
    And a clap error for a missing argument
    When I localize the clap error
    Then the localized text matches the baseline clap output

  Scenario: CLI parse missing subcommand uses translations
    Given a clap-aware localizer
    When I parse a demo command without a subcommand
    Then the localized text contains clap-error-missing-subcommand
    And the localized text contains greet
    And the localized text contains take-leave

  Scenario: CLI parse missing argument falls back without translations
    Given a noop localizer
    When I parse a demo command missing an argument
    Then the localized text matches the baseline clap output

  Scenario: Help text resolves through derive-generated identifiers
    Given a derived configuration struct with a localization catalogue
    When the command line is parsed with a Fluent localizer
    Then the help text resolves through the derive-generated identifiers

  Scenario: A newly covered parser error is translated
    Given a localized command requiring equals for its option
    When I parse that option without equals
    Then the error kind is NoEquals
    And the en-US no-equals translation is rendered

  Scenario: Missing-input help remains clap output
    Given a localized command showing help when arguments are absent
    When I parse no arguments
    Then the error kind is DisplayHelpOnMissingArgumentOrSubcommand
    And the localizer is not called
    And the output and exit status equal stock clap

  Scenario: An unavailable override retains usable English
    Given an incomplete locale and the embedded en-US catalogue
    When I trigger a newly covered parser error
    Then the embedded en-US translation is rendered without formatting errors
