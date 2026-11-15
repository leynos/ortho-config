Feature: Recover all missing configuration from one diagnostic
  The loader names every absent required value once, with the routes that can
  supply it, and recovers when a value arrives through any supported source.

  Scenario: Correct a root value and two nested required values together
    Given a schema-enabled configuration with reference, database.host and database.port required
    And no source supplies those values
    When the combined configuration is loaded
    Then one missing-values error names all three paths exactly once
    And every suggested route is accepted by its provider
    When valid values are supplied using the reported routes
    Then the next load succeeds without another missing-values error

  Scenario: Optional and supplied values are not reported
    Given an absent optional parent and a supplied required value
    When the configuration with an optional parent is loaded
    Then neither value nor the optional parent's children are reported

  Scenario: Invalid nested input is not absence
    Given a scalar where a nested configuration object is required
    When the configuration with a scalar parent is loaded
    Then the existing invalid-value error is retained

  Scenario: A missing top-level value is diagnosed and recovered from the command line
    Given a configuration requiring a reference
    When the configuration is loaded without the required value
    Then exactly one diagnostic names the path "reference"
    And the diagnostic advertises the command-line flag "--reference <value>"
    When the reference is supplied on the command line
    Then the configuration loads successfully

  Scenario: Optional values are not reported
    Given a configuration requiring a reference
    When the configuration is loaded with a supplied reference and an absent optional value
    Then no missing-value diagnostic is produced
    And the configured default port is applied

  Scenario: A missing nested value is recovered from the environment
    Given a nested configuration with an empty database table
    When the nested configuration is loaded
    Then exactly one nested diagnostic names the path "database.host"
    And the nested diagnostic advertises the environment variable "MRQN_DATABASE__HOST"
    When the database host is supplied through the environment
    Then the nested configuration loads successfully

  Scenario: Invalid nested input is not reported as absence
    Given a nested configuration whose database value is not an object
    When the nested configuration is loaded
    Then a configuration error is returned instead of a missing-value diagnostic