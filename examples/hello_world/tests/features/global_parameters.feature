Feature: Global parameters govern greetings
  The hello_world example demonstrates global flags, switches, and arrays.

  Scenario: Using defaults produces a friendly greeting
    When I run the hello world example
    Then the command succeeds
    And stdout contains "Hello, World!"

  Scenario: Excited delivery shouts the greeting
    When I run the hello world example with arguments "--excited true"
    Then the command succeeds
    And stdout contains "HELLO, WORLD!"

  Scenario: Quiet delivery softens the message
    When I run the hello world example with arguments "--quiet true"
    Then the command succeeds
    And stdout contains "Hello, World..."

  Scenario: Conflicting delivery modes are rejected
    When I run the hello world example with arguments "--excited true --quiet true"
    Then the command fails
    And stderr contains "cannot combine --excited with --quiet"

  Scenario: Custom salutations are honoured
    When I run the hello world example with arguments "-s Hi -s there -r team"
    Then the command succeeds
    And stdout contains "Hi there, team!"
