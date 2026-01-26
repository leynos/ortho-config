Feature: cargo-orthohelp bridge pipeline

  Scenario: Generate per-locale IR JSON
    Given a temporary output directory
    When I run cargo-orthohelp with cache for the fixture
    Then the output contains localised IR JSON for en-US
    And the output contains localised IR JSON for fr-FR
    When I run cargo-orthohelp with no-build for the fixture
    Then the output contains localised IR JSON for en-US
