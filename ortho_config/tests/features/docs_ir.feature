Feature: OrthoConfigDocs IR
  Documentation metadata must emit deterministic identifiers so external
  tooling can localise and format CLI help without inspecting clap output.

  Scenario: Deterministic documentation IDs are emitted
    When I request the docs metadata
    Then the IR version is 1.1
    And the about id is demo-app.about
    And the help id for field log_level is demo-app.fields.log_level.help
    And the long help id for field log_level is demo-app.fields.log_level.long_help
    And the environment variable for field log_level is APP_LOG_LEVEL
    And the windows module name is Demo
    And the windows metadata includes common parameters
    And the windows metadata does not split subcommands
