#![warn(clippy::all, rust_2018_idioms)]

use slotmap::HopSlotMap;
use std::collections::HashMap;
mod feature;
pub use feature::Feature;
mod handler;
pub use handler::Handler;
pub mod tools;

const MAX_HOVER_DISTANCE: f32 = 160.0;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Viewport {
    pub fn screen_to_point(&self, p: egui::Pos2) -> egui::Pos2 {
        egui::Pos2 {
            x: self.zoom * p.x + self.x,
            y: self.zoom * p.y + self.y,
        }
    }
    pub fn translate_point(&self, p: egui::Pos2) -> egui::Pos2 {
        egui::Pos2 {
            x: (p.x - self.x) / self.zoom,
            y: (p.y - self.y) / self.zoom,
        }
    }
    pub fn translate_rect(&self, r: egui::Rect) -> egui::Rect {
        egui::Rect {
            min: self.translate_point(r.min),
            max: self.translate_point(r.max),
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            zoom: 1.,
        }
    }
}

/// CommandHandler maps commands issued by tools into actions performed on
/// the drawing.
pub trait CommandHandler<F, C>: std::fmt::Debug + Sized
where
    F: DrawingFeature,
    C: std::fmt::Debug + Sized,
{
    fn handle(&mut self, drawing: &mut Data<F>, c: C);
}

/// Data stores state about the drawing and what it is composed of.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Data<F>
where
    F: DrawingFeature,
{
    pub features: HopSlotMap<slotmap::DefaultKey, F>,
    pub vp: Viewport,

    pub selected_map: HashMap<slotmap::DefaultKey, usize>,
}

impl<F: DrawingFeature> Default for Data<F> {
    fn default() -> Self {
        Self {
            features: HopSlotMap::default(),
            vp: Viewport::default(),
            selected_map: HashMap::default(),
        }
    }
}

impl<F: DrawingFeature> Data<F> {
    pub fn find_point_at(&self, p: egui::Pos2) -> Option<slotmap::DefaultKey> {
        for (k, v) in self.features.iter() {
            if v.bb(self).center().distance_sq(p) < 0.0001 {
                return Some(k);
            }
        }
        None
    }

    pub fn find_screen_feature(&self, hp: egui::Pos2) -> Option<(slotmap::DefaultKey, F)> {
        let mut closest: Option<(slotmap::DefaultKey, f32, bool)> = None;
        for (k, v) in self.features.iter() {
            let is_point = v.is_point();

            // Points get a head-start in terms of being considered closer, so
            // they are chosen over a line segment when hovering near the end of
            // a line segment.
            let dist = if is_point {
                v.screen_dist(self, hp, &self.vp) - (MAX_HOVER_DISTANCE / 2.)
            } else {
                v.screen_dist(self, hp, &self.vp)
            };

            if dist < MAX_HOVER_DISTANCE {
                closest = Some(
                    closest
                        .map(|c| if dist < c.1 { (k, dist, is_point) } else { c })
                        .unwrap_or((k, dist, is_point)),
                );
            }
        }

        match closest {
            Some((k, _dist, _is_point)) => Some((k, self.features.get(k).unwrap().clone())),
            None => None,
        }
    }

    pub fn delete_feature(&mut self, k: slotmap::DefaultKey) -> bool {
        self.selected_map.remove(&k);

        match self.features.remove(k) {
            Some(_v) => {
                // Find and also remove any features dependent on what we just removed.
                let to_delete: std::collections::HashSet<slotmap::DefaultKey> = self
                    .features
                    .iter()
                    .map(|(k2, v2)| {
                        let dependent_deleted = v2
                            .depends_on()
                            .into_iter()
                            .filter_map(|d| d.map(|d| d == k))
                            .reduce(|p, f| p || f);

                        match dependent_deleted {
                            Some(true) => Some(k2),
                            _ => None,
                        }
                    })
                    .filter_map(|d| d)
                    .collect();

                for k in to_delete {
                    self.delete_feature(k);
                }

                true
            }
            None => false,
        }
    }

    pub fn selection_delete(&mut self) {
        let elements: Vec<_> = self.selected_map.drain().map(|(k, _)| k).collect();
        for k in elements {
            self.delete_feature(k);
        }
    }

    pub fn select_feature(&mut self, feature: &slotmap::DefaultKey, select: bool) {
        let currently_selected = self.selected_map.contains_key(feature);
        if currently_selected && !select {
            self.selected_map.remove(feature);
        } else if !currently_selected && select {
            let next_idx = self.selected_map.values().fold(0, |acc, x| acc.max(*x)) + 1;
            self.selected_map.insert(feature.clone(), next_idx);
        }
    }

    pub fn select_features_in_rect(&mut self, rect: egui::Rect, select: bool) {
        let keys: Vec<_> = self
            .features
            .iter()
            .filter(|(_, v)| rect.contains_rect(v.bb(self)))
            .map(|(k, _)| k)
            .collect();

        for k in keys.into_iter() {
            self.select_feature(&k, select);
        }
    }

    pub fn selection_clear(&mut self) {
        self.selected_map.clear();
    }

    pub fn feature_selected(&self, feature: &slotmap::DefaultKey) -> bool {
        self.selected_map.get(feature).is_some()
    }
}

/// Colors describes the colors with which different elements should be styled.
#[derive(Clone, Debug, Default)]
pub struct Colors {
    point: egui::Color32,
    line: egui::Color32,
    selected: egui::Color32,
    hover: egui::Color32,
    text: egui::Color32,
}

#[derive(Clone, Debug)]
pub struct PaintParams {
    selected: bool,
    hovered: bool,

    rect: egui::Rect,
    vp: Viewport,
    colors: Colors,
    font_id: egui::FontId,
}

/// DrawingFeature describes elements which can make up a drawing.
pub trait DrawingFeature: std::fmt::Debug + Clone + Sized {
    fn is_point(&self) -> bool;
    fn depends_on(&self) -> [Option<slotmap::DefaultKey>; 2];
    fn bb(&self, drawing: &Data<Self>) -> egui::Rect;
    fn screen_dist(&self, drawing: &Data<Self>, hp: egui::Pos2, vp: &Viewport) -> f32;
    fn paint(
        &self,
        drawing: &Data<Self>,
        k: slotmap::DefaultKey,
        params: &PaintParams,
        painter: &egui::Painter,
    );
}

/// ToolController implements tools which can be used to manipulate the drawing.
pub trait ToolController: std::fmt::Debug + Sized {
    type Command: std::fmt::Debug + Sized;
    type Features: DrawingFeature;

    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        hp: Option<egui::Pos2>,
        hf: &Option<(slotmap::DefaultKey, Self::Features)>,
        response: &egui::Response,
    ) -> Option<Self::Command>;
    fn paint(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        hp: Option<egui::Pos2>,
        params: &PaintParams,
    );
}

/// Widget implements the egui drawing widget.
#[derive(Debug)]
pub struct Widget<'a, F, TC, CH>
where
    F: DrawingFeature,
    TC: ToolController,
    CH: CommandHandler<F, TC::Command>,
{
    pub drawing: &'a mut Data<F>,
    pub tools: &'a mut TC,
    pub handler: &'a mut CH,
}

impl<'a, F, TC, CH> Widget<'a, F, TC, CH>
where
    F: DrawingFeature,
    TC: ToolController<Features = F>,
    CH: CommandHandler<F, TC::Command>,
{
    pub fn new(drawing: &'a mut Data<F>, handler: &'a mut CH, tools: &'a mut TC) -> Self {
        Self {
            drawing,
            tools,
            handler,
        }
    }

    // handle_inputs returns the bounds of the in-progress selection, if any.
    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        hp: Option<egui::Pos2>,
        hf: &Option<(slotmap::DefaultKey, F)>,
        response: &egui::Response,
    ) -> Option<egui::Rect> {
        // Handle: zooming
        if let Some(hp) = hp {
            // println!("hp: {:?}", hp);
            use std::ops::Add;
            let hp = hp.add(-egui::Vec2 {
                x: response.rect.width() / 2.,
                y: response.rect.height() / 2.,
            });

            let scroll_delta = ui.input(|i| i.scroll_delta);
            if scroll_delta.y != 0. {
                let m = self.drawing.vp.translate_point(hp);

                self.drawing.vp.zoom *= f32::exp(-1. * scroll_delta.y * 0.1823216 / 230.);
                if self.drawing.vp.zoom < 0.05 {
                    self.drawing.vp.zoom = 0.05;
                }
                let after = self.drawing.vp.translate_point(hp);

                // println!("{:?} => {:?}", m, after);

                self.drawing.vp.x -= (m.x - after.x) * self.drawing.vp.zoom;
                self.drawing.vp.y -= (m.y - after.y) * self.drawing.vp.zoom;
            }
        }

        // Handle: panning
        if response.dragged_by(egui::PointerButton::Secondary) {
            let egui::Vec2 { x, y } = response.drag_delta();
            self.drawing.vp.x -= x * self.drawing.vp.zoom;
            self.drawing.vp.y -= y * self.drawing.vp.zoom;
        }

        // Handle: dragging a box to select
        let current_drag = if let Some(hp) = hp {
            let state_id = ui.make_persistent_id("select_box_start");
            let drag_start_pos = if response.drag_started_by(egui::PointerButton::Primary) {
                let dp = self.drawing.vp.screen_to_point(hp);
                ui.memory_mut(|mem| mem.data.insert_temp(state_id, dp));
                Some(dp)
            } else {
                ui.memory(|mem| mem.data.get_temp(state_id))
            };

            let released = response.drag_released_by(egui::PointerButton::Primary);
            match (drag_start_pos, released) {
                (Some(drag_start), true) => {
                    let s =
                        egui::Rect::from_two_pos(drag_start, self.drawing.vp.screen_to_point(hp));
                    if s.area() > 200. {
                        let shift_held = ui.input(|i| i.modifiers.shift);
                        if !shift_held {
                            self.drawing.selection_clear();
                        }
                        self.drawing.select_features_in_rect(s, true);
                    }
                    ui.memory_mut(|mem| mem.data.remove::<egui::Pos2>(state_id));
                    None
                }
                (Some(drag_start), false) => {
                    let s =
                        egui::Rect::from_two_pos(drag_start, self.drawing.vp.screen_to_point(hp));
                    if s.area() > 200. {
                        Some(s)
                    } else {
                        None
                    }
                }
                (None, _) => None,
            }
        } else {
            None
        };

        // Handle: clicks altering selection
        if hp.is_some() && response.clicked_by(egui::PointerButton::Primary) {
            let shift_held = ui.input(|i| i.modifiers.shift);

            // feature clicked: add-to or replace selection
            if let Some((k, _)) = hf {
                if !shift_held {
                    self.drawing.selection_clear();
                }
                self.drawing
                    .select_feature(k, !self.drawing.feature_selected(k));
            } else if !shift_held {
                // empty space clicked, clear selection.
                self.drawing.selection_clear();
            }
        }

        // Handle: escape clears collection
        if hp.is_some()
            && self.drawing.selected_map.len() > 0
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.drawing.selection_clear();
        }

        // Handle: Ctrl-A selects all
        if hp.is_some() && ui.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.ctrl) {
            for k in self.drawing.features.keys() {
                if !self.drawing.selected_map.contains_key(&k) {
                    let next_idx = self
                        .drawing
                        .selected_map
                        .values()
                        .fold(0, |acc, x| acc.max(*x))
                        + 1;

                    self.drawing.selected_map.insert(k, next_idx);
                }
            }
        }

        // Handle: delete selection
        if hp.is_some()
            && self.drawing.selected_map.len() > 0
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
        {
            self.drawing.selection_delete();
        }

        current_drag
    }

    fn draw(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        hp: Option<egui::Pos2>,
        hf: Option<(slotmap::DefaultKey, F)>,
        current_drag: Option<egui::Rect>,
        base_params: &PaintParams,
    ) {
        for point_pass in [true, false] {
            for (k, v) in self.drawing.features.iter() {
                if point_pass != v.is_point() {
                    continue;
                }

                let hovered = hf.as_ref().map(|(hk, _dist)| hk == &k).unwrap_or(false)
                    || current_drag
                        .as_ref()
                        .map(|dr| dr.contains_rect(v.bb(self.drawing)))
                        .unwrap_or(false);
                let selected = self.drawing.selected_map.get(&k).is_some();

                let pp = PaintParams {
                    hovered,
                    selected,
                    ..base_params.clone()
                };
                v.paint(self.drawing, k, &pp, painter);
            }
        }

        if let Some(current_drag) = current_drag {
            let screen_rect = self.drawing.vp.translate_rect(current_drag);
            painter.rect_filled(
                screen_rect.shrink(1.),
                egui::Rounding::ZERO,
                egui::Color32::from_white_alpha(20),
            );
            painter.rect_stroke(
                screen_rect,
                egui::Rounding::ZERO,
                egui::Stroke {
                    width: 1.,
                    color: egui::Color32::WHITE,
                },
            );
        }

        self.tools.paint(ui, painter, hp, &base_params);

        self.draw_debug(ui, painter, hp, &base_params);
    }

    fn draw_debug(
        &mut self,
        _ui: &egui::Ui,
        painter: &egui::Painter,
        hp: Option<egui::Pos2>,
        base_params: &PaintParams,
    ) {
        let debug_text = painter.layout_no_wrap(
            format!("{:?}", self.drawing.vp).to_owned(),
            base_params.font_id.clone(),
            base_params.colors.text,
        );
        if let Some(hover) = hp {
            if (egui::Rect {
                min: egui::Pos2 {
                    x: base_params.rect.left(),
                    y: base_params.rect.bottom() - debug_text.size().y - 2.,
                },
                max: egui::Pos2 {
                    x: base_params.rect.left() + debug_text.size().x,
                    y: base_params.rect.bottom(),
                },
            })
            .contains(hover)
            {
                self.drawing.vp.x = -base_params.rect.width() / 2.;
                self.drawing.vp.y = -base_params.rect.height() / 2.;
                self.drawing.vp.zoom = 1.;
            }
        }
        painter.add(egui::Shape::galley(
            egui::Pos2 {
                x: base_params.rect.left(),
                y: base_params.rect.bottom() - debug_text.size().y - 2.,
            },
            debug_text,
        ));
    }

    pub fn show(mut self, ui: &mut egui::Ui) -> DrawResponse {
        use egui::Sense;
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size(),
            Sense::click_and_drag().union(Sense::hover()),
        );
        ui.set_clip_rect(rect);

        // First-frame initialization
        let state_id = ui.make_persistent_id("drawing");
        let has_init = ui
            .memory_mut(|mem| mem.data.get_temp::<bool>(state_id))
            .unwrap_or(false);
        if !has_init {
            self.drawing.vp.x = -rect.width() / 2.;
            self.drawing.vp.y = -rect.height() / 2.;
            ui.memory_mut(|mem| mem.data.insert_temp(state_id, true));
        }

        // Find hover feature, if any
        let hp = response.hover_pos();
        let hf = hp
            .map(|hp| self.drawing.find_screen_feature(hp))
            .unwrap_or(None);

        // Handle input
        let current_drag = if let Some(c) = self.tools.handle_input(ui, hp, &hf, &response) {
            self.handler.handle(self.drawing, c);
            None
        } else {
            self.handle_input(ui, hp, &hf, &response)
        };

        let base_params = PaintParams {
            rect,
            vp: self.drawing.vp.clone(),
            colors: Colors {
                point: egui::Color32::GREEN,
                line: egui::Color32::LIGHT_GRAY,
                selected: egui::Color32::RED,
                hover: egui::Color32::YELLOW,
                text: ui.visuals().text_color(),
            },
            font_id: egui::TextStyle::Body.resolve(ui.style()),

            selected: false,
            hovered: false,
        };
        let painter = ui.painter();

        self.draw(ui, painter, hp, hf, current_drag, &base_params);
        DrawResponse {}
    }
}

pub struct DrawResponse {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplifications() {}
}
