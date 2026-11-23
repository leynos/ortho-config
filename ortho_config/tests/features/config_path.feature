Feature: Custom config path flag
  Scenario: load with custom config path flag
    Given an alternate config file with rule "from_file"
    When the config is loaded with custom flag "--config" "alt.toml"
    Then the loaded rules are "from_file"

  Scenario: default flag is rejected
    Given an alternate config file with rule "from_file"
    When the config is loaded with custom flag "--config-path" "alt.toml"
    Then config loading fails with a CLI parsing error

  Scenario: a missing required configuration path fails despite an automatic fallback
    Given a valid automatic configuration file with rule "auto"
    When the config is loaded with custom flag "--config" pointing at a missing file
    Then config loading fails with a required file error

  Scenario: a missing environment-selected configuration path fails despite an automatic fallback
    Given a valid automatic configuration file with rule "auto"
    And the environment-selected configuration path does not exist
    When the config is loaded with the injected environment
    Then config loading fails with a required file error

  Scenario: an unset environment-selected configuration path allows automatic discovery
    Given a valid automatic configuration file with rule "auto"
    When the config is loaded with the injected environment
    Then the loaded rules are "auto"
