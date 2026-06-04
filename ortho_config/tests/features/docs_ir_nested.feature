Feature: Nested OrthoConfigDocs IR
  Nested documentation metadata must expose the whole command tree so
  external tooling can render each observable command without inspecting
  clap output.

  Scenario: Nested tree exposes every top-level command in declaration order
    Given the nested CLI fixture
    When I request the nested docs metadata
    Then the nested top-level commands are greet, version, admin

  Scenario: Greet command exposes its recipient field and example
    Given the nested CLI fixture
    When I request the nested docs metadata
    Then command "greet" contains field "recipient"
    And command "greet" field "recipient" has default "String :: from(\"World\")"
    And command "greet" has example "nested-app greet --recipient Ada"

  Scenario: Version command exposes no fields
    Given the nested CLI fixture
    When I request the nested docs metadata
    Then command "version" exposes no fields

  Scenario: Admin command exposes audit and grant-access subcommands in order
    Given the nested CLI fixture
    When I request the nested docs metadata
    Then command "admin" contains nested commands audit, grant-access

  Scenario: Admin command exposes Windows wrapper metadata that splits subcommands into functions
    Given the nested CLI fixture
    When I request the nested docs metadata
    Then command "admin" exposes Windows wrapper metadata
    And command "admin" splits subcommands into functions

  Scenario: Greet command exposes no Windows wrapper metadata
    Given the nested CLI fixture
    When I request the nested docs metadata
    Then command "greet" exposes no Windows wrapper metadata
