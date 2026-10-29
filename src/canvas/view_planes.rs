//! `Canvas` drill-down: switching the active plane (bpmn-js
//! `canvas.setRootElement`) with one viewport per plane, the drill-down
//! buttons, the breadcrumbs bar and the collapse/expand toggle.

use gpui::{Context, Window};

use super::Canvas;
use crate::canvas::breadcrumbs::Breadcrumbs;
use crate::canvas::drill_button::DrillButton;
use crate::canvas::drilldown::{button_rect, crumbs, targets};
use crate::canvas::palette;
use crate::model::{ElementId, ElementKind, Waypoint};
use crate::modeling::Command;

impl Canvas {
    /// Opens the plane of sub-process `owner` (`None` = root), saving the
    /// current plane's viewport and restoring the target's.
    pub(super) fn open_plane(&mut self, owner: Option<&ElementId>, cx: &mut Context<Self>) {
        let doc = &mut self.docs[self.active];
        doc.viewports.insert(doc.plane.clone(), doc.viewport);
        self.defs_mut().open_plane(owner);
        self.sync_plane();
        cx.notify();
    }

    /// Follows a plane change made by the model (undo/redo re-open the
    /// plane a command ran on): swaps viewports and drops plane-local state.
    pub(super) fn sync_plane(&mut self) {
        let owner = self.docs[self.active].definitions.plane_owner().cloned();
        if owner == self.docs[self.active].plane {
            return;
        }

        let doc = &mut self.docs[self.active];
        doc.viewports.insert(doc.plane.clone(), doc.viewport);
        doc.viewport = doc.viewports.get(&owner).copied().unwrap_or_default();
        doc.plane = owner;
        doc.selection.clear();
        self.hover = None;
        self.replacing = false;
        self.connect_source = None;
    }

    /// Collapses or expands the selected sub-process (replace menu row).
    pub(super) fn on_toggle_collapse(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.replacing = false;

        if let Some(id) = self.single_selected() {
            self.execute(Command::ToggleCollapse { id }, cx);
        }

        cx.notify();
    }

    /// Whether the single selected sub-process is expanded; `None` when the
    /// selection is not one sub-process (no collapse/expand row).
    pub(super) fn collapse_state(&self) -> Option<bool> {
        let id = self.single_selected()?;
        let element = self.docs[self.active].definitions.get(&id)?;
        if !matches!(element.kind, ElementKind::SubProcess { .. }) {
            return None;
        }

        Some(self.docs[self.active].definitions.shape(&id)?.is_expanded == Some(true))
    }

    /// Drill-down buttons of the collapsed sub-processes on screen.
    pub(super) fn drill_buttons(&self, cx: &mut Context<Self>) -> Vec<DrillButton> {
        targets(
            &self.docs[self.active].definitions,
            &self.docs[self.active].selection,
        )
        .into_iter()
        .map(|(id, bounds)| {
            let rect = button_rect(&self.docs[self.active].viewport.bounds_to_screen(bounds));
            let open = cx.listener(move |this, _: &(), _, cx| this.open_plane(Some(&id), cx));
            DrillButton::new(rect, move |window, cx| open(&(), window, cx))
        })
        .collect()
    }

    /// Breadcrumbs bar right of the palette, only while drilled down.
    pub(super) fn breadcrumbs(&self, cx: &mut Context<Self>) -> Option<Breadcrumbs> {
        let path = crumbs(&self.docs[self.active].definitions);
        if path.len() < 2 {
            return None;
        }

        let right = palette::layout()
            .iter()
            .map(|(_, b)| b.x + b.w)
            .fold(0.0, f64::max);
        let origin = Waypoint {
            x: palette::ORIGIN.x + right + 20.0,
            y: palette::ORIGIN.y,
        };
        let pick = cx
            .listener(|this, plane: &Option<ElementId>, _, cx| this.open_plane(plane.as_ref(), cx));
        Some(Breadcrumbs::new(origin, path, move |plane, window, cx| {
            pick(&plane.cloned(), window, cx)
        }))
    }
}
