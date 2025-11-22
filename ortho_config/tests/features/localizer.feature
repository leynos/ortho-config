Feature: Localizer trait
  Localisation helpers must offer predictable fallback behaviour so CLI
  surfaces can opt into translations incrementally.

  Scenario: No-op localiser falls back to defaults
    Given a noop localiser
    When I request id cli.about with fallback Default CLI about
    Then the localised text is Default CLI about

  Scenario: Subject-aware localiser formats Fluent arguments
    Given a subject-aware localiser
    When I request id cli.about for subject Ada Lovelace
    Then the localised text is Hola, Ada Lovelace! (cli.about)
