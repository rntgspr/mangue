---
human_revised: false
plan: maintenance-soc-canvas-text
status: draft
date: 2026-10-26
summary: Record the TextInput layout cache ownership change for canvas.
---

# Delta draft — maintenance-soc-canvas-text

## specs/modeler/canvas.md

### Added Requirements

- The `TextInput` paint-time layout cache MUST be private and updated through its own `record_layout` operation.
