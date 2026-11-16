# Build workflow skill

Fixture manifest for the skill-manifest validator. The child `build` command
declares `--color` explicitly even though the root command also offers it: an
effective surface spells out every option a command really accepts rather than
relying on inheritance.

## Command index

```json
[
  {
    "path": ["tool", "build"],
    "flags": ["json", "color"]
  }
]
```

## Checked examples

```json
[
  {
    "manifest_id": "build-workflow",
    "command_reference_ordinal": 1,
    "path": ["tool", "build"],
    "options": [
      {
        "name": "json",
        "value": null,
        "source_ordinal": 1
      }
    ],
    "positionals": [],
    "source_ordinal": 1,
    "extraction": "checked"
  },
  {
    "manifest_id": "build-workflow",
    "command_reference_ordinal": 1,
    "path": ["tool", "build"],
    "options": [
      {
        "name": "color",
        "value": "always",
        "source_ordinal": 1
      }
    ],
    "positionals": [],
    "source_ordinal": 2,
    "extraction": "checked"
  }
]
```
