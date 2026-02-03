Feature: cargo-orthohelp roff man page generation

  Scenario: Generate man page from fixture
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format man for the fixture
    Then the output contains a man page for orthohelp_fixture
    And the man page for orthohelp_fixture contains section NAME
    And the man page for orthohelp_fixture contains section SYNOPSIS
    And the man page for orthohelp_fixture contains section DESCRIPTION
    And the man page for orthohelp_fixture contains section OPTIONS

  Scenario: Man page uses correct section number
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format man and section 5 for the fixture
    Then the output contains a man page at section 5 for orthohelp_fixture

  Scenario: Generate all formats
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with format all for the fixture
    Then the output contains localized IR JSON for en-US
    And the output contains a man page for orthohelp_fixture
