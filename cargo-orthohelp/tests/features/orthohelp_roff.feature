Feature: cargo-orthohelp roff man page generation

  Scenario: Generate man page from fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format man for the fixture
    Then the output contains a man page for fixture
    And the man page for fixture contains section NAME
    And the man page for fixture contains section SYNOPSIS
    And the man page for fixture contains section DESCRIPTION
    And the man page for fixture contains section OPTIONS

  Scenario: Man page uses correct section number
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format man and section 5 for the fixture
    Then the output contains a man page at section 5 for fixture

  Scenario: Generate man pages for multiple locales
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format man for en-US and fr-FR
    Then the output contains a localized man page for en-US and fixture
    And the output contains a localized man page for fr-FR and fixture

  Scenario: Generate all formats
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format all for the fixture
    Then the output contains localized IR JSON for en-US
    And the output contains a man page for fixture
    And the output contains a PowerShell module named FixtureHelp
