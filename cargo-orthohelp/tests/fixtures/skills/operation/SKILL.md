# Operation skill

Fixture manifest for the skill-manifest validator. A consumer would extract
these claims from the prose below; the JSON blocks stand in for that extraction
so the fixture stays reviewable.

## Command index

```json
[
  {
    "path": ["tool", "records", "get"],
    "flags": ["json"]
  }
]
```

## Checked examples

```json
[
  {
    "manifest_id": "operation",
    "command_reference_ordinal": 1,
    "path": ["tool", "records", "get"],
    "options": [
      {
        "name": "json",
        "value": null,
        "source_ordinal": 1
      }
    ],
    "positionals": ["42"],
    "source_ordinal": 1,
    "extraction": "checked"
  }
]
```
