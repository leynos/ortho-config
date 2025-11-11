Feature: rstest-bdd scaffolding for hello_world
  Scenario: Resolving a custom recipient
    Given the hello world scenario state is reset
    When I load the hello world CLI with recipient Tamsin
    Then the recipient name resolves to Tamsin
