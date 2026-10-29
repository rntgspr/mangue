---
human_revised: false
plan: maintenance-soc-io
status: draft
date: 2026-10-26
summary: Draft specification changes for application-layer BPMN persistence composition.
targets: [platform]
---

# Delta draft

- `modeler/io.md`: XML import and file loading return the parsed model without modeling normalization; import errors use their Display text.
- `modeler/modeling.md`: subprocess compatibility and edge orthogonalization are composed by the canvas persistence boundary after loading a file; code import applies subprocess compatibility alone.
- `modeler/canvas.md`: `canvas::persist` composes file open, code import, and ordered XML save.
