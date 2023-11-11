#![warn(clippy::all, rust_2018_idioms)]

pub mod l;

mod data;
pub use data::{Data, Viewport};
mod feature;
pub use feature::{Feature, FeatureKey, FeatureMeta};
mod constraints;
pub use constraints::{Constraint, ConstraintKey, ConstraintMeta};
pub mod handler;
pub use handler::Handler;
pub mod tools;

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

#[derive(Clone, Debug, Copy)]
enum DragState {
    SelectBox(egui::Pos2),
    Feature(FeatureKey, egui::Vec2),
}

#[derive(Clone, Debug, Copy)]
enum Input {
    Selection(egui::Rect),
    FeatureDrag(FeatureKey, egui::Pos2),
}

/// Widget implements the egui drawing widget.
#[derive(Debug)]
pub struct Widget<'a> {
    pub drawing: &'a mut Data,
    pub tools: &'a mut tools::Toolbar,
    pub handler: &'a mut Handler,
}

impl<'a> Widget<'a> {
    pub fn new(
        drawing: &'a mut Data,
        handler: &'a mut Handler,
        tools: &'a mut tools::Toolbar,
    ) -> Self {
        Self {
            drawing,
            tools,
            handler,
        }
    }

    // handle_inputs returns the what the user is interacting with in the drawing, if any.
    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        hp: Option<egui::Pos2>,
        hf: &Option<(FeatureKey, Feature)>,
        response: &egui::Response,
    ) -> Option<Input> {
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

        // Handle: selection, dragging
        let current_input = if let Some(hp) = hp {
            let select_id = ui.make_persistent_id("select_box_start");
            let drag_state = if response.drag_started_by(egui::PointerButton::Primary) {
                match hf {
                    // dragging a box to select
                    None => {
                        let state = DragState::SelectBox(self.drawing.vp.screen_to_point(hp));
                        ui.memory_mut(|mem| mem.data.insert_temp(select_id, state));
                        Some(state)
                    }
                    // Dragging a point
                    Some((fk, Feature::Point(_, px, py))) => {
                        let offset =
                            self.drawing.vp.screen_to_point(hp) - egui::Pos2::new(*px, *py);
                        let state = DragState::Feature(*fk, offset);
                        ui.memory_mut(|mem| mem.data.insert_temp(select_id, state));
                        Some(state)
                    }
                    Some(_) => None,
                }
            } else {
                ui.memory(|mem| mem.data.get_temp(select_id))
            };

            let released = response.drag_released_by(egui::PointerButton::Primary);
            match (drag_state, released) {
                (Some(DragState::SelectBox(drag_start)), true) => {
                    let s =
                        egui::Rect::from_two_pos(drag_start, self.drawing.vp.screen_to_point(hp));
                    if s.area() > 200. {
                        let shift_held = ui.input(|i| i.modifiers.shift);
                        if !shift_held {
                            self.drawing.selection_clear();
                        }
                        self.drawing.select_features_in_rect(s, true);
                    }
                    ui.memory_mut(|mem| mem.data.remove::<DragState>(select_id));
                    None
                }
                (Some(DragState::SelectBox(drag_start)), false) => {
                    let s =
                        egui::Rect::from_two_pos(drag_start, self.drawing.vp.screen_to_point(hp));
                    if s.area() > 200. {
                        Some(Input::Selection(s))
                    } else {
                        None
                    }
                }

                (Some(DragState::Feature(fk, offset)), _) => {
                    if released {
                        ui.memory_mut(|mem| mem.data.remove::<DragState>(select_id));
                    }
                    let new_pos = self.drawing.vp.screen_to_point(hp) - offset;
                    self.drawing.move_feature(fk, new_pos);
                    Some(Input::FeatureDrag(fk, new_pos))
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
            self.drawing.select_all();
        }

        // Handle: delete selection
        if hp.is_some()
            && self.drawing.selected_map.len() > 0
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
        {
            self.drawing.selection_delete();
        }

        current_input
    }

    fn draw(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        hp: Option<egui::Pos2>,
        hf: Option<(FeatureKey, Feature)>,
        response: &egui::Response,
        current_input: Option<Input>,
        base_params: &PaintParams,
    ) {
        // Draw features, points first
        for point_pass in [true, false] {
            for (k, v) in self.drawing.features_iter() {
                if point_pass != v.is_point() {
                    continue;
                }

                let hovered = hf.as_ref().map(|(hk, _dist)| hk == &k).unwrap_or(false)
                    || current_input
                        .as_ref()
                        .map(|dr| {
                            if let Input::Selection(b) = dr {
                                b.contains_rect(v.bb(self.drawing))
                            } else {
                                false
                            }
                        })
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

        // Draw constraints
        for (k, v) in self.drawing.constraints_iter() {
            v.paint(self.drawing, k, &base_params, painter);
        }

        if let Some(Input::Selection(current_drag)) = current_input {
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

        self.tools.paint(ui, painter, response, hp, &base_params);

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
        let current_input = if let Some(c) = self.tools.handle_input(ui, hp, &hf, &response) {
            self.handler.handle(self.drawing, self.tools, c);
            None
        } else {
            self.handle_input(ui, hp, &hf, &response)
        };

        let base_params = PaintParams {
            rect,
            vp: self.drawing.vp.clone(),
            colors: Colors {
                point: egui::Color32::GREEN,
                line: if ui.visuals().dark_mode {
                    egui::Color32::LIGHT_GRAY
                } else {
                    egui::Color32::DARK_GRAY
                },
                selected: egui::Color32::RED,
                hover: egui::Color32::YELLOW,
                text: ui.visuals().text_color(),
            },
            font_id: egui::TextStyle::Body.resolve(ui.style()),

            selected: false,
            hovered: false,
        };
        let painter = ui.painter();

        self.draw(ui, painter, hp, hf, &response, current_input, &base_params);
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
