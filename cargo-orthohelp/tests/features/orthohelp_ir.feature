Feature: cargo-orthohelp bridge pipeline

  Scenario: Generate per-locale IR JSON
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with cache for the fixture
    Then the output contains localized IR JSON for en-US
    And the output contains localized IR JSON for fr-FR
    And the cached IR deserializes into the schema
    When I rerun cargo-orthohelp with cache for the fixture
    Then the cached IR matches its recorded content
    When I invoke cargo-orthohelp in no-build mode for the fixture
    Then the output contains localized IR JSON for en-US

  Scenario: Generate IR JSON with explicit legacy format
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format ir for the fixture
    Then the output contains localized IR JSON for en-US

  Scenario: No-build fails without cache
    Given a temporary output directory
    And the orthohelp cache is empty
    When I invoke cargo-orthohelp in no-build mode for the fixture
    Then the command fails due to missing cache

  Scenario: Rebuilding replaces stale IR and reports cache I/O failures
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with cache for the fixture
    And I corrupt the cached IR
    And I invoke cargo-orthohelp in cache-free rebuild mode for the fixture
    Then the command succeeds
    And the cached IR matches its recorded content
    When I replace the cached IR with a directory
    And I attempt cargo-orthohelp with cache for the fixture
    Then the command fails with a cache I/O error
    When I replace the cached IR directory with a read-only file
    And I invoke cargo-orthohelp in cache-free rebuild mode for the fixture
    Then the command fails with a cache I/O error
    When I restore cached IR file permissions
