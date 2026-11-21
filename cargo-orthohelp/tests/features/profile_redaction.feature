Feature: Profile metadata keeps sensitive values private
  Scenario: Protected metadata is omitted from every output format
    Given a profile-enabled command with secret and reference-only fields
    And their defaults, enum members and examples contain distinct sentinels
    When context JSON and human documentation are generated
    Then public field descriptors remain visible
    And context JSON declares both classifications
    And no protected sentinel appears in any generated artefact

  Scenario: Parent examples cannot disclose a protected child
    Given a nested command tree with a secret child field
    And parent and sibling command examples contain its sentinel
    When documentation is generated
    Then every command-level example collection is empty

  Scenario: Public profile names do not expose profile contents
    Given two catalogues with the same validated public names
    And every profile body differs between the catalogues
    When each consumer attaches only the names to its context
    Then both serialized contexts are identical
    And neither context contains profile bodies

  Scenario: Ordinary metadata retains its behaviour
    Given a command with no marked fields
    When context and documentation are generated
    Then its defaults and examples retain their existing representation

  Scenario: Unsupported profiles still redact without inventing a declaration
    Given a command that marks a secret field without profile support
    When context JSON and documentation are generated for the unsupported fixture
    Then the context declares profiles unsupported
    And the protected default is absent from the context
    And no unsupported sentinel appears in any generated artefact

  Scenario: Hidden and file-only protected fields stay recorded but not invocable
    Given a profile-enabled command with hidden and file-only protected fields
    When context JSON is generated
    Then the redaction list names both fields
    And neither field is an invocable input

  Scenario: The cache stores only sanitized IR
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with cache for the profile-redaction fixture
    Then the cached IR carries no protected sentinel
    When I rerun cargo-orthohelp with cache for the profile-redaction fixture
    Then the cached IR is reused

  Scenario: An entry the current cache key does not name is never reused
    Given a temporary output directory
    And the orthohelp cache is empty
    And a cache entry under an unrelated key retains a protected sentinel
    When I run cargo-orthohelp with no-build for the profile-redaction fixture
    Then the command fails due to missing cache
    And the unrelated cache entry was not rewritten

  Scenario: A hand-edited cache entry is projected before it is rendered
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with cache for the profile-redaction fixture
    And I hand-edit the cached IR to retain a protected default
    When I run cargo-orthohelp with no-build man output for the profile-redaction fixture
    Then no rendered artefact carries the hand-edited sentinel

  Scenario: Duplicate protected records stop the run with a diagnostic
    Given a temporary output directory
    And the orthohelp cache is empty
    When I run cargo-orthohelp with cache for the profile-redaction fixture
    And I hand-edit the cached IR to duplicate a protected record
    When I run cargo-orthohelp with no-build context output for the profile-redaction fixture
    Then the command fails naming the duplicate redaction
