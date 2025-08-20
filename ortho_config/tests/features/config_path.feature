Feature: Custom config path flag
  Scenario: rename config path flag
    Given an alternate config file with rule "from_file"
    When the config is loaded with custom flag "--config" "alt.toml"
    Then the loaded rules are "from_file"
