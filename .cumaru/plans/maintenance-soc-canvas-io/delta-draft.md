---
human_revised: false
plan: maintenance-soc-canvas-io
status: draft
date: 2026-10-26
summary: Draft canvas specification changes for file system helper ownership.
---

# Delta draft

- `modeler/canvas.md`: `src/files` owns file extension checks, dialog start directory selection, file URL decoding, and mutable folder tree lookup; canvas and app entry call these helpers.
