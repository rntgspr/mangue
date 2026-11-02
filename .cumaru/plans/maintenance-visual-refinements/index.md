---
human_revised: false
scope: [modeler/canvas, modeler/code-editor]
status: in-progress
summary: Iteratively refine the modeler interface through individually scoped visual adjustments.
targets: [platform]
---

# Visual refinements

## Overview

Make visual adjustments iteratively as requested. Add a task and its acceptance criteria for each agreed adjustment, expanding scope only when the affected spec owner is known. Work in the current branch; no branch is required for this plan.

## Acceptance Criteria (EARS / RFC 2119)

- WHILE a document is open THE SYSTEM SHALL render a 36px tab strip with visible labels, selection, dirty markers and close controls.
- ON macOS THE SYSTEM SHALL integrate the document tabs into the window title area while retaining usable native window controls and window dragging.
- WHILE a document tab is hovered THE SYSTEM SHALL show its close button; otherwise the button SHALL be hidden, and tab labels SHALL use a smaller font.
- WHEN a tab close button is shown THE SYSTEM SHALL display a Lucide X icon in a square control.
- WHEN a tab close button is shown THE SYSTEM SHALL inset it from the trailing tab edge by the same distance as the filename from the leading edge.
- WHILE document tabs are shown THE SYSTEM SHALL use a horizontal edge inset of 8px on each side.
- WHILE the document header is visible THE SYSTEM SHALL inset its contents by 16px horizontally.
- WHEN code mode opens with one diagram element selected THE SYSTEM SHALL reveal and highlight its XML element without selecting text.
- WHEN a single element is selected THE SYSTEM SHALL show its XML ancestor chain in the document breadcrumb, and selecting an ancestor crumb SHALL select that element.
- WHEN an XML element is highlighted THE SYSTEM SHALL highlight the entire element, including its closing tag and nested content.
- WHILE the document breadcrumb is visible THE SYSTEM SHALL present it without a surrounding button-like box and distinguish navigable items as links.
- WHEN exactly one diagram element is selected THE SYSTEM SHALL show its XML ID in the document breadcrumb; multiple selected elements SHALL be represented by their count.
- WHEN additional visual adjustments are agreed THE PLAN SHALL record each as a separate task with observable acceptance criteria and verification.

## Plan / DAG

| Task | Title | Status | Depends on |
|------|-------|--------|------------|
| [T1](t1.md) | Increase tab height | in-progress | — |
| [T2](t2.md) | Integrate the macOS title area | pending | T1 |
| [T3](t3.md) | Refine tab labels and close controls | in-progress | — |
| [T4](t4.md) | Use a square icon-only close button | in-progress | — |
| [T5](t5.md) | Align tab edge spacing | in-progress | — |
| [T6](t6.md) | Reduce tab horizontal padding | in-progress | — |
| [T7](t7.md) | Show selection in the document breadcrumb | in-progress | — |
| [T8](t8.md) | Increase document header padding | in-progress | — |
| [T9](t9.md) | Reveal selected element in XML | in-progress | — |
| [T10](t10.md) | Navigate selected element ancestors | in-progress | — |
| [T11](t11.md) | Highlight the complete XML element | in-progress | — |
| [T12](t12.md) | Restyle document breadcrumb links | in-progress | — |

## Out of scope

- Unrequested visual changes and speculative features.
- Branch creation.

## Risks

- Native window controls, drag behavior and full-screen layout require real macOS verification.
- Changes to shared chrome must preserve document navigation and other platforms' window behavior.
