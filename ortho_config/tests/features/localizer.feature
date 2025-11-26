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
