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
