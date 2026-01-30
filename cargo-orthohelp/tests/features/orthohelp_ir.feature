Feature: cargo-orthohelp bridge pipeline

  Scenario: Generate per-locale IR JSON
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with cache for the fixture
    Then the output contains localized IR JSON for en-US
    And the output contains localized IR JSON for fr-FR
    And the cached IR deserializes into the schema
    When I rerun cargo-orthohelp with cache for the fixture
    Then the cached IR is reused
    When I run cargo-orthohelp with no-build for the fixture
    Then the output contains localized IR JSON for en-US

  Scenario: No-build fails without cache
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with no-build for the fixture
    Then the command fails due to missing cache
