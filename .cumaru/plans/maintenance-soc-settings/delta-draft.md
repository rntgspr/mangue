---
human_revised: false
plan: maintenance-soc-settings
status: draft
date: 2026-10-26
summary: Draft settings and canvas spec changes for recent file menu observation.
---

# Delta draft

- `modeler/settings.md`: recent-file persistence updates the `RecentFiles` global without calling canvas.
- `modeler/canvas.md`: action installation observes `RecentFiles` and rebuilds the app menus; the initial menu reflects the global already installed at startup.
