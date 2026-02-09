Feature: Custom config path flag
  Scenario: load with custom config path flag
    Given an alternate config file with rule "from_file"
    When the config is loaded with custom flag "--config" "alt.toml"
    Then the loaded rules are "from_file"

  Scenario: default flag is rejected
    Given an alternate config file with rule "from_file"
    When the config is loaded with custom flag "--config-path" "alt.toml"
    Then config loading fails with a CLI parsing error
