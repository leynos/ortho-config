Feature: Merge error routing
  Merge phase errors route through OrthoError::Merge

  Scenario: Invalid type produces Merge error
    Given a merge layer with port set to "not_a_number"
    When the layers are merged
    Then a Merge error variant is returned

  Scenario: Valid layers produce successful config
    Given a merge layer with port set to 8080
    When the layers are merged
    Then the merged config has port 8080
