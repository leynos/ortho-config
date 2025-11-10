Feature: rstest-bdd scaffolding for ortho_config
  Scenario: Loading configuration via the rstest-bdd canary
    Given the canary scenario state is reset
    When I load the canary config with level 9
    Then the canary level equals 9
