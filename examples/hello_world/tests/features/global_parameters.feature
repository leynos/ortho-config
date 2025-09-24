Feature: Global parameters govern greetings
  The hello_world example demonstrates global flags, switches, and arrays.

  Scenario: Using defaults produces a friendly greeting
    When I run the hello world example with arguments "greet"
    Then the command succeeds
    And stdout contains "Hello, World!"

  Scenario: Excited delivery shouts the greeting
    When I run the hello world example with arguments "--is-excited greet"
    Then the command succeeds
    And stdout contains "HELLO, WORLD!"

  Scenario: Quiet delivery softens the message
    When I run the hello world example with arguments "--is-quiet greet"
    Then the command succeeds
    And stdout contains "Hello, World..."

  Scenario: Conflicting delivery modes are rejected
    When I run the hello world example with arguments "--is-excited --is-quiet greet"
    Then the command fails
    And stderr contains "cannot combine --is-excited with --is-quiet"

  Scenario: Custom salutations are honoured
    When I run the hello world example with arguments "-s Hi -s there -r team greet"
    Then the command succeeds
    And stdout contains "Hi there, team!"

  Scenario: Printing a preamble before greeting
    When I run the hello world example with arguments "greet --preamble 'Good morning'"
    Then the command succeeds
    And stdout contains "Good morning"

  Scenario: Taking leave summarises the farewell
    When I run the hello world example with arguments "--is-excited take-leave --gift biscuits --remind-in 15 --channel email --wave"
    Then the command succeeds
    And stdout contains "HELLO, WORLD!"
    And stdout contains "leaves biscuits"
