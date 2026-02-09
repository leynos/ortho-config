Feature: PowerShell help generation

  Scenario: Generate PowerShell help for the fixture
    Given a temporary output directory
    When I run cargo-orthohelp with format ps for the fixture
    Then the output contains a PowerShell module named "FixtureHelp"
    And the PowerShell help for "FixtureHelp" includes command "fixture"
    And the PowerShell about topic for "FixtureHelp" exists
