#![warn(clippy::all, rust_2018_idioms)]

pub mod l;

mod data;
pub use data::{group::*, Data, Hover, SelectedElement, SerializedDrawing, Viewport};
mod feature;
pub use feature::{Feature, FeatureKey, FeatureMeta, SerializedFeature};
mod constraints;
pub use constraints::{
    Axis, Constraint, ConstraintKey, ConstraintMeta, DimensionDisplay, SerializedConstraint,
};
pub mod handler;
mod system;
pub use handler::Handler;
pub mod tools;

pub const CONSTRUCTION_IMG: egui::ImageSource<'static> =
    egui::include_image!("../../assets/emoji_u1f6a7.png");

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
    Point(FeatureKey, egui::Vec2),
    Line(FeatureKey, egui::Vec2, egui::Vec2, egui::Vec2), // p1, p2, offset
    Circle(FeatureKey, egui::Vec2, egui::Vec2),           // center, offset
    Constraint(ConstraintKey, egui::Vec2),
    EditingLineLength(ConstraintKey),
    PointRightClick(FeatureKey, egui::Pos2),
    LineRightClick(FeatureKey, egui::Pos2),
}

#[derive(Clone, Debug, Copy)]
enum Input {
    Selection(egui::Rect),
    FeatureDrag(FeatureKey, egui::Pos2),
    ConstraintDrag(ConstraintKey, egui::Pos2),
    EditingLineLength(ConstraintKey),
    PointRightClick(FeatureKey, egui::Pos2),
    LineRightClick(FeatureKey, egui::Pos2),
}

/// Widget implements the egui drawing widget.
#[derive(Debug)]
pub struct Widget<'a> {
    pub drawing: &'a mut Data,
    pub tools: &'a mut tools::Toolbar,
    pub handler: &'a mut Handler,

    length_ticks: Vec<f32>,
    center_next_frame: bool,
    autozoom_next_frame: bool,
}

impl<'a> Widget<'a> {
    pub fn new(
        drawing: &'a mut Data,
        handler: &'a mut Handler,
        tools: &'a mut tools::Toolbar,
    ) -> Self {
        let length_ticks = Vec::with_capacity(8);
        let center_next_frame = false;
        let autozoom_next_frame = false;

        Self {
            drawing,
            tools,
            handler,
            length_ticks,
            center_next_frame,
            autozoom_next_frame,
        }
    }

    // handle_inputs returns the what the user is interacting with in the drawing, if any.
    fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        hp: Option<egui::Pos2>,
        hover: &Hover,
        response: &mut egui::Response,
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
        let state_id = egui::Id::new("_drawing_input_state");
        let current_input = if let Some(hp) = hp {
            let drag_state = match (
                hover,
                response.drag_started_by(egui::PointerButton::Primary),
                response.double_clicked_by(egui::PointerButton::Primary),
                response.clicked_by(egui::PointerButton::Secondary),
                self.drawing.drag_features_enabled,
                self.drawing.drag_dimensions_enabled,
            ) {
                // dragging a box to select
                (Hover::None, true, false, false, _, _) => {
                    let state = DragState::SelectBox(self.drawing.vp.screen_to_point(hp));
                    ui.memory_mut(|mem| mem.data.insert_temp(state_id, state));
                    Some(state)
                }
                // Dragging a point
                (
                    Hover::Feature {
                        k,
                        feature: Feature::Point(_, px, py),
                    },
                    true,
                    false,
                    false,
                    true,
                    _,
                ) => {
                    let offset = self.drawing.vp.screen_to_point(hp) - egui::Pos2::new(*px, *py);
                    let state = DragState::Point(*k, offset);
                    ui.memory_mut(|mem| mem.data.insert_temp(state_id, state));
                    Some(state)
                }
                // Dragging a line
                (
                    Hover::Feature {
                        k,
                        feature: Feature::LineSegment(..),
                    },
                    true,
                    false,
                    false,
                    true,
                    _,
                ) => {
                    let (a, b) = self.drawing.get_line_points(*k).unwrap();

                    let offset = self.drawing.vp.screen_to_point(hp).to_vec2();
                    let state = DragState::Line(*k, a.to_vec2(), b.to_vec2(), offset);
                    ui.memory_mut(|mem| mem.data.insert_temp(state_id, state));
                    Some(state)
                }
                // Dragging a circle
                (
                    Hover::Feature {
                        k,
                        feature: Feature::Circle(_, center, ..),
                    },
                    true,
                    false,
                    false,
                    true,
                    _,
                ) => {
                    let center_pt = if let Some(Feature::Point(_, x, y, ..)) =
                        self.drawing.features.get(*center)
                    {
                        egui::Vec2::new(*x, *y)
                    } else {
                        unreachable!()
                    };

                    let offset = self.drawing.vp.screen_to_point(hp).to_vec2();
                    let state = DragState::Circle(*k, center_pt, offset);
                    ui.memory_mut(|mem| mem.data.insert_temp(state_id, state));
                    Some(state)
                }
                // Dragging a LineLength or CircleRadius constraint reference
                (Hover::Constraint { k, constraint }, true, false, false, _, true)
                    if matches!(constraint, Constraint::CircleRadius(..))
                        || matches!(constraint, Constraint::LineLength(..)) =>
                {
                    let offset = constraint.dimension_pos(self.drawing).unwrap() - hp.to_vec2();
                    let state = DragState::Constraint(*k, offset.to_vec2());
                    ui.memory_mut(|mem| mem.data.insert_temp(state_id, state));
                    Some(state)
                }
                // Double-clicking a LineLength constraint reference
                (
                    Hover::Constraint {
                        k,
                        constraint: Constraint::LineLength(_, _, _, _, _dd),
                    },
                    false,
                    true,
                    false,
                    _,
                    _,
                ) => {
                    if let Some(Constraint::LineLength(..)) = self.drawing.constraints.get(*k) {
                        let state = DragState::EditingLineLength(*k);
                        ui.memory_mut(|mem| {
                            mem.data.insert_temp(state_id, state);
                        });
                        Some(state)
                    } else {
                        unreachable!();
                    }
                }
                // Right-click on a point
                (
                    Hover::Feature {
                        k,
                        feature: Feature::Point(..),
                    },
                    false,
                    false,
                    true,
                    _,
                    _,
                ) => {
                    let state = DragState::PointRightClick(*k, self.drawing.vp.screen_to_point(hp));
                    ui.memory_mut(|mem| {
                        mem.data.insert_temp(state_id, state);
                    });
                    Some(state)
                }
                // Right-click on a line
                (
                    Hover::Feature {
                        k,
                        feature: Feature::LineSegment(..),
                    },
                    false,
                    false,
                    true,
                    _,
                    _,
                ) => {
                    let state = DragState::LineRightClick(*k, self.drawing.vp.screen_to_point(hp));
                    ui.memory_mut(|mem| {
                        mem.data.insert_temp(state_id, state);
                    });
                    Some(state)
                }

                (Hover::Constraint { .. }, true, false, false, _, _) => None,
                (_, _, _, _, _, _) => ui.memory(|mem| mem.data.get_temp::<DragState>(state_id)),
            };

            let released = response.drag_released_by(egui::PointerButton::Primary);
            match (drag_state, released) {
                (Some(DragState::SelectBox(drag_start)), true) => {
                    if egui::Rect::from_two_pos(self.drawing.vp.translate_point(drag_start), hp)
                        .area()
                        > 200.
                    {
                        let shift_held = ui.input(|i| i.modifiers.shift);
                        if !shift_held {
                            self.drawing.selection_clear();
                        }
                        self.drawing.select_features_in_rect(
                            egui::Rect::from_two_pos(
                                drag_start,
                                self.drawing.vp.screen_to_point(hp),
                            ),
                            true,
                        );
                    }
                    ui.memory_mut(|mem| mem.data.remove::<DragState>(state_id));
                    None
                }
                (Some(DragState::SelectBox(drag_start)), false) => {
                    if egui::Rect::from_two_pos(self.drawing.vp.translate_point(drag_start), hp)
                        .area()
                        > 200.
                    {
                        Some(Input::Selection(egui::Rect::from_two_pos(
                            drag_start,
                            self.drawing.vp.screen_to_point(hp),
                        )))
                    } else {
                        None
                    }
                }

                (Some(DragState::Point(fk, offset)), _) => {
                    if released {
                        ui.memory_mut(|mem| mem.data.remove::<DragState>(state_id));
                    }
                    let new_pos = self.drawing.vp.screen_to_point(hp) - offset;
                    self.drawing.move_point(fk, new_pos);
                    response.mark_changed();
                    Some(Input::FeatureDrag(fk, new_pos))
                }
                (Some(DragState::Line(fk, p1, p2, offset)), _) => {
                    if released {
                        ui.memory_mut(|mem| mem.data.remove::<DragState>(state_id));
                    }

                    let (fk1, fk2) = if let Some(Feature::LineSegment(_, fk1, fk2)) =
                        self.drawing.features.get(fk)
                    {
                        (fk1.clone(), fk2.clone())
                    } else {
                        unreachable!();
                    };

                    let p1_pos = self.drawing.vp.screen_to_point(hp) - offset + p1;
                    self.drawing.move_point(fk1, p1_pos);
                    let p2_pos = self.drawing.vp.screen_to_point(hp) - offset + p2;
                    self.drawing.move_point(fk2, p2_pos);

                    response.mark_changed();
                    Some(Input::FeatureDrag(fk, p1_pos))
                }
                (Some(DragState::Circle(fk, center, offset)), _) => {
                    if released {
                        ui.memory_mut(|mem| mem.data.remove::<DragState>(state_id));
                    }

                    let c_fk =
                        if let Some(Feature::Circle(_, c_fk, ..)) = self.drawing.features.get(fk) {
                            c_fk.clone()
                        } else {
                            unreachable!();
                        };

                    let np = self.drawing.vp.screen_to_point(hp) - offset + center;
                    self.drawing.move_point(c_fk, np);

                    response.mark_changed();
                    Some(Input::FeatureDrag(fk, np))
                }

                (Some(DragState::Constraint(ck, offset)), _) => {
                    if released {
                        ui.memory_mut(|mem| mem.data.remove::<DragState>(state_id));
                    }
                    self.drawing.move_constraint(ck, hp + offset);
                    Some(Input::ConstraintDrag(ck, hp))
                }

                (Some(DragState::EditingLineLength(ck)), _) => {
                    if response.clicked() && matches!(hover, Hover::None) {
                        ui.memory_mut(|mem| mem.data.remove::<DragState>(state_id));
                    }
                    Some(Input::EditingLineLength(ck))
                }

                (Some(DragState::PointRightClick(k, p)), _) => Some(Input::PointRightClick(k, p)),
                (Some(DragState::LineRightClick(k, p)), _) => Some(Input::LineRightClick(k, p)),
                (None, _) => None,
            }
        } else {
            // Cases where we want to keep track even if the cursor is in another window
            match ui.memory(|mem| mem.data.get_temp::<DragState>(state_id)) {
                Some(DragState::PointRightClick(k, p)) => Some(Input::PointRightClick(k, p)),
                Some(DragState::LineRightClick(k, p)) => Some(Input::LineRightClick(k, p)),
                Some(DragState::EditingLineLength(ck)) => Some(Input::EditingLineLength(ck)),
                _ => None,
            }
        };

        // All clicks get keyboard focus.
        // println!("focus-w: {:?}", response.ctx.memory(|mem| mem.focus()));
        if response.clicked() && !response.lost_focus() {
            self.set_focus(ui, response);
        }

        // Handle: clicks altering selection
        if hp.is_some()
            && response.clicked_by(egui::PointerButton::Primary)
            && !matches!(current_input, Some(Input::EditingLineLength(_)))
        {
            let shift_held = ui.input(|i| i.modifiers.shift);

            // feature clicked: add-to or replace selection
            if let Hover::Feature { k, .. } = hover {
                if !shift_held {
                    self.drawing.selection_clear();
                }
                self.drawing
                    .select_feature(*k, !self.drawing.feature_selected(*k));
            } else if let Hover::Constraint { k, .. } = hover {
                if !shift_held {
                    self.drawing.selection_clear();
                }
                self.drawing
                    .select_constraint(*k, !self.drawing.constraint_selected(*k));
            } else if !shift_held {
                // empty space clicked, clear selection.
                self.drawing.selection_clear();
            }
        }

        // Handle: escape clears collection - cant use focus check here?
        if hp.is_some()
            && self.drawing.selected_map.len() > 0
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.drawing.selection_clear();
        }

        // Handle: Ctrl-A selects all
        if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.ctrl) {
            self.drawing.select_all();
        }

        // Handle: delete selection
        if response.has_focus()
            && hp.is_some()
            && self.drawing.selected_map.len() > 0
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
        {
            self.drawing.selection_delete();
        }

        current_input
    }

    fn set_focus(&self, ui: &egui::Ui, response: &egui::Response) {
        ui.memory_mut(|mem| {
            mem.request_focus(response.id);
            mem.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    escape: true,
                    ..Default::default()
                },
            );
        });
    }

    fn length_tick_for_amt(length_ticks: &mut Vec<f32>, amt: f32) -> usize {
        for (i, val) in length_ticks.iter().enumerate() {
            if (val - amt).abs() < 0.00001 {
                return i;
            }
        }

        let i = length_ticks.len();
        length_ticks.push(amt);
        i
    }

    fn draw(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        hp: Option<egui::Pos2>,
        hover: Hover,
        response: &egui::Response,
        current_input: Option<Input>,
        base_params: &PaintParams,
    ) {
        self.length_ticks.clear();

        // Draw features, points first
        for point_pass in [true, false] {
            for (k, v) in self.drawing.features_iter() {
                if point_pass != v.is_point() {
                    continue;
                }

                let hovered = match hover {
                    Hover::Feature { k: hk, .. } => hk == k,
                    _ => false,
                } || current_input
                    .as_ref()
                    .map(|dr| {
                        if let Input::Selection(b) = dr {
                            b.contains_rect(v.bb(self.drawing))
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);

                let selected = self
                    .drawing
                    .selected_map
                    .get(&SelectedElement::Feature(k))
                    .is_some();

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
            let hovered = match hover {
                Hover::Constraint { k: hk, .. } => hk == k,
                _ => false,
            };
            let selected = self
                .drawing
                .selected_map
                .get(&SelectedElement::Constraint(k))
                .is_some();

            let pp = PaintParams {
                hovered,
                selected,
                ..base_params.clone()
            };
            v.paint(self.drawing, k, &pp, painter);
        }

        // Draw equal ticks
        for (_k, v) in self.drawing.constraints_iter() {
            match v {
                Constraint::LineLengthsEqual(_, l1, l2, None) => {
                    let (a, b) = self.drawing.get_line_points(*l1).unwrap();
                    let tick = Widget::length_tick_for_amt(&mut self.length_ticks, a.distance(b));

                    crate::l::draw::length_tick(a, b, tick, painter, &base_params);

                    let (a, b) = self.drawing.get_line_points(*l2).unwrap();
                    crate::l::draw::length_tick(a, b, tick, painter, &base_params);
                }
                _ => {}
            }
        }

        match current_input {
            Some(Input::Selection(current_drag)) => {
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

            Some(Input::PointRightClick(k, p)) => {
                self.show_point_context_menu(ui, k, p);
            }
            Some(Input::LineRightClick(k, p)) => {
                self.show_line_context_menu(ui, k, p);
            }

            Some(Input::EditingLineLength(ck)) => {
                self.show_line_dimension_popover(ui, ck);
            }

            Some(Input::FeatureDrag(_, _)) | Some(Input::ConstraintDrag(_, _)) | None => {}
        };

        self.tools
            .paint(ui, painter, response, hp, &base_params, self.drawing);

        self.draw_debug(ui, painter, hp, &base_params);
    }

    fn show_line_dimension_popover(&mut self, ui: &egui::Ui, ck: ConstraintKey) {
        if let Some(Constraint::LineLength(_, fk, _, _, dd)) = self.drawing.constraints.get(ck) {
            if let Some(Feature::LineSegment(_, f1, f2)) = self.drawing.features.get(*fk) {
                let (a, b) = match (
                    self.drawing.features.get(*f1).unwrap(),
                    self.drawing.features.get(*f2).unwrap(),
                ) {
                    (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                        (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                    }
                    _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                };

                let reference = egui::Vec2::from((dd.x, dd.y));
                let t = (a - b).angle() + reference.angle();
                let reference_screen = self.drawing.vp.translate_point(a.lerp(b, 0.5))
                    + egui::Vec2::angled(t) * reference.length();

                let mut changed: Option<()> = None;
                if let Some(Constraint::LineLength(_, _, d, ..)) =
                    self.drawing.constraints.get_mut(ck)
                {
                    egui::Area::new(egui::Id::new("dimension_popup"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(reference_screen)
                        .constrain(true)
                        .pivot(egui::Align2::CENTER_CENTER)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

                                let dv =
                                    ui.add_sized([75., text_height * 1.4], egui::DragValue::new(d));

                                if dv.changed() {
                                    if *d < 0. {
                                        *d = 0.;
                                    }
                                    changed = Some(());
                                }
                                if dv.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Escape))
                                {
                                    ui.memory_mut(|mem| {
                                        mem.data.remove::<DragState>(egui::Id::new(
                                            "_drawing_input_state",
                                        ))
                                    });
                                }
                            });
                        });
                }

                if changed.is_some() {
                    self.drawing.changed_in_ui();
                }
            };
        };
    }

    fn show_point_context_menu(&mut self, ui: &egui::Ui, k: FeatureKey, p: egui::Pos2) {
        let mut command: Option<handler::ToolResponse> = None;
        let mut show_more = ui.memory(|m| {
            m.data
                .get_temp::<bool>(egui::Id::new("show_more").with(k))
                .unwrap_or(false)
        });

        let Data {
            features,
            constraints,
            ..
        } = self.drawing;

        if let Some(Feature::Point(meta, ..)) = features.get_mut(k) {
            egui::Area::new(egui::Id::new("drawing_ctx_menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(self.drawing.vp.translate_point(p) + egui::Vec2::new(4., 4.))
                .constrain(true)
                .interactable(true)
                .movable(false)
                .show(ui.ctx(), |ui| {
                    ui.allocate_ui(egui::Vec2::new(250., 550.), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let new_show_wizard = if show_more {
                                    if ui.button("⏷").clicked() {
                                        Some(false)
                                    } else {
                                        None
                                    }
                                } else {
                                    if ui.button("⏵").clicked() {
                                        Some(true)
                                    } else {
                                        None
                                    }
                                };
                                if let Some(new_val) = new_show_wizard {
                                    ui.memory_mut(|m| {
                                        m.data.insert_temp(
                                            egui::Id::new("show_more").with(k),
                                            new_val,
                                        )
                                    });
                                    show_more = new_val;
                                }

                                use slotmap::Key;
                                ui.label(format!("Point {:?}", k.data()));
                                ui.add_space(12.);

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::TOP),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new("⊗")
                                                    .fill(egui::Color32::DARK_RED),
                                            )
                                            .clicked()
                                        {
                                            command = Some(handler::ToolResponse::Delete(k));
                                        }
                                        ui.add_space(4.);

                                        ui.add(egui::Checkbox::without_text(
                                            &mut meta.construction,
                                        ));
                                        ui.add(
                                            egui::Image::new(crate::CONSTRUCTION_IMG).rounding(5.0),
                                        );
                                    },
                                );
                            });

                            // If constrained to fixed co-ordinates, show those coords
                            if let Some(Constraint::Fixed(_, _, x, y)) = constraints
                                .get_using_feature_and_type(
                                    &k,
                                    std::mem::discriminant(&Constraint::Fixed(
                                        ConstraintMeta::default(),
                                        k,
                                        0.,
                                        0.,
                                    )),
                                )
                            {
                                ui.add_space(4.);
                                ui.horizontal(|ui| {
                                    ui.label("Fixed coordinates");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::TOP),
                                        |ui| {
                                            ui.label(format!("({}, {})", x, y));
                                        },
                                    );
                                });
                            }

                            if show_more {
                                ui.separator();

                                ui.label("Wizard: Generate points array");
                                ui.indent("points array", |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        let text_height =
                                            egui::TextStyle::Body.resolve(ui.style()).size;

                                        ui.columns(2, |columns| {
                                            columns[0].add_sized(
                                                [75., text_height * 1.4],
                                                egui::Label::new("No. points"),
                                            );
                                            columns[1].add_sized(
                                                [25., text_height * 1.4],
                                                egui::DragValue::new(
                                                    &mut self.drawing.menu_state.array_wizard_count,
                                                ),
                                            );

                                            columns[0].add_sized(
                                                [75., text_height * 1.4],
                                                egui::Label::new("Direction"),
                                            );
                                            egui::ComboBox::from_id_source(
                                                "points_array_wizard_direction",
                                            )
                                            .selected_text(format!(
                                                "{:?}",
                                                self.drawing.menu_state.array_wizard_direction
                                            ))
                                            .show_ui(
                                                &mut columns[1],
                                                |ui| {
                                                    ui.selectable_value(
                                                        &mut self
                                                            .drawing
                                                            .menu_state
                                                            .array_wizard_direction,
                                                        crate::data::Direction::Up,
                                                        "Up",
                                                    );
                                                    ui.selectable_value(
                                                        &mut self
                                                            .drawing
                                                            .menu_state
                                                            .array_wizard_direction,
                                                        crate::data::Direction::Down,
                                                        "Down",
                                                    );
                                                    ui.selectable_value(
                                                        &mut self
                                                            .drawing
                                                            .menu_state
                                                            .array_wizard_direction,
                                                        crate::data::Direction::Left,
                                                        "Left",
                                                    );
                                                    ui.selectable_value(
                                                        &mut self
                                                            .drawing
                                                            .menu_state
                                                            .array_wizard_direction,
                                                        crate::data::Direction::Right,
                                                        "Right",
                                                    );
                                                },
                                            );

                                            columns[0].add_sized(
                                                [75., text_height * 1.4],
                                                egui::Label::new("Spacing"),
                                            );
                                            columns[1].add_sized(
                                                [25., text_height * 1.4],
                                                egui::DragValue::new(
                                                    &mut self
                                                        .drawing
                                                        .menu_state
                                                        .array_wizard_separation,
                                                )
                                                .speed(0.05)
                                                .clamp_range(0.00..=1000.0)
                                                .suffix("mm"),
                                            );
                                        });
                                    });
                                    ui.horizontal(|ui| {
                                        ui.add_space(12.);
                                        if ui.button("Execute").clicked() {
                                            command = Some(handler::ToolResponse::ArrayWizard(
                                                k,
                                                p.to_vec2(),
                                                self.drawing.menu_state.clone(),
                                            ));
                                        };
                                    });
                                });
                            }
                        });
                    });
                });
        }

        if let Some(c) = command {
            self.handler.handle(self.drawing, self.tools, c);
        }
    }

    fn show_line_context_menu(&mut self, ui: &egui::Ui, k: FeatureKey, p: egui::Pos2) {
        let mut command: Option<handler::ToolResponse> = None;
        let mut show_more = ui.memory(|m| {
            m.data
                .get_temp::<bool>(egui::Id::new("show_more").with(k))
                .unwrap_or(false)
        });

        let Data {
            features,
            constraints,
            ..
        } = self.drawing;

        if let Some(Feature::LineSegment(meta, ..)) = features.get_mut(k) {
            egui::Area::new(egui::Id::new("drawing_ctx_menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(self.drawing.vp.translate_point(p) + egui::Vec2::new(4., 4.))
                .constrain(true)
                .interactable(true)
                .movable(false)
                .show(ui.ctx(), |ui| {
                    ui.allocate_ui(egui::Vec2::new(250., 550.), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let new_show_wizard = if show_more {
                                    if ui.button("⏷").clicked() {
                                        Some(false)
                                    } else {
                                        None
                                    }
                                } else {
                                    if ui.button("⏵").clicked() {
                                        Some(true)
                                    } else {
                                        None
                                    }
                                };
                                if let Some(new_val) = new_show_wizard {
                                    ui.memory_mut(|m| {
                                        m.data.insert_temp(
                                            egui::Id::new("show_more").with(k),
                                            new_val,
                                        )
                                    });
                                    show_more = new_val;
                                }

                                use slotmap::Key;
                                ui.label(format!("Line {:?}", k.data()));
                                ui.add_space(12.);

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::TOP),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new("⊗")
                                                    .fill(egui::Color32::DARK_RED),
                                            )
                                            .clicked()
                                        {
                                            command = Some(handler::ToolResponse::Delete(k));
                                        }
                                        ui.add_space(4.);

                                        ui.add(egui::Checkbox::without_text(
                                            &mut meta.construction,
                                        ));
                                        ui.add(
                                            egui::Image::new(crate::CONSTRUCTION_IMG).rounding(5.0),
                                        );
                                    },
                                );
                            });

                            // If constrained to a certain length, show that
                            if let Some(Constraint::LineLength(_, _, length, cardinality, ..)) =
                                constraints.get_using_feature_and_type(
                                    &k,
                                    std::mem::discriminant(&Constraint::LineLength(
                                        ConstraintMeta::default(),
                                        k,
                                        0.0,
                                        None,
                                        constraints::DimensionDisplay::default(),
                                    )),
                                )
                            {
                                ui.add_space(4.);
                                ui.horizontal(|ui| {
                                    ui.label("Length");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::TOP),
                                        |ui| {
                                            ui.label(format!("{:.3}mm", length));
                                        },
                                    );
                                });
                                if let Some((c, _)) = cardinality {
                                    ui.horizontal(|ui| {
                                        ui.label("Cardinality");
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::TOP),
                                            |ui| {
                                                ui.label(match c {
                                                    constraints::Axis::LeftRight => "Horizontal",
                                                    constraints::Axis::TopBottom => "Vertical",
                                                });
                                            },
                                        );
                                    });
                                }
                            }

                            // If constrained to be horizontal/vertical, show that
                            if let Some(Constraint::LineAlongCardinal(_, _, axis, ..)) = constraints
                                .get_using_feature_and_type(
                                    &k,
                                    std::mem::discriminant(&Constraint::LineAlongCardinal(
                                        ConstraintMeta::default(),
                                        k,
                                        constraints::Axis::TopBottom,
                                    )),
                                )
                            {
                                ui.add_space(4.);
                                ui.horizontal(|ui| {
                                    ui.label("Cardinality");
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::TOP),
                                        |ui| {
                                            ui.label(match axis {
                                                constraints::Axis::LeftRight => "Horizontal",
                                                constraints::Axis::TopBottom => "Vertical",
                                            });
                                        },
                                    );
                                });
                            }

                            if show_more {
                                ui.separator();
                            }
                        });
                    });
                });
        }

        if let Some(c) = command {
            self.handler.handle(self.drawing, self.tools, c);
        }
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

    pub fn center(&mut self) {
        self.center_next_frame = true;
    }

    pub fn autozoom(&mut self) {
        self.autozoom_next_frame = true;
    }

    pub fn show(mut self, ui: &mut egui::Ui) -> DrawResponse {
        use egui::Sense;
        let (rect, mut response) = ui.allocate_exact_size(
            ui.available_size(),
            Sense {
                click: true,
                drag: true,
                focusable: true,
            },
        );
        ui.set_clip_rect(rect);

        // First-frame initialization
        let state_id = ui.make_persistent_id("drawing");
        let has_init = ui
            .memory_mut(|mem| mem.data.get_temp::<bool>(state_id))
            .unwrap_or(false);
        if !has_init {
            if self.drawing.vp.eq(&Viewport::default()) {
                self.center_next_frame = true;
            }
            ui.memory_mut(|mem| {
                mem.data.insert_temp(state_id, true);
                mem.request_focus(response.id); // request focus initially
            });
        }

        if self.autozoom_next_frame {
            let bb = self.drawing.bounds();
            let (x_r, y_r) = (
                1.35 / (rect.width() / bb.width()),
                1.25 / (rect.height() / bb.height()),
            );
            self.drawing.vp.zoom = x_r.max(y_r);
        }
        if self.center_next_frame {
            let bounds = self.drawing.bounds();
            self.drawing.vp.x = -rect.width() / 2. * self.drawing.vp.zoom + bounds.center().x;
            self.drawing.vp.y = -rect.height() / 2. * self.drawing.vp.zoom + bounds.center().y;
        }

        // Find hover feature, if any
        let hp = response.hover_pos();
        let hover = hp
            .map(|hp| self.drawing.find_screen_hover(hp))
            .unwrap_or(Hover::None);

        // Handle input
        let current_input = if let Some(c) = self.tools.handle_input(ui, hp, &hover, &response) {
            self.handler.handle(self.drawing, self.tools, c);
            self.set_focus(ui, &response);
            None
        } else {
            self.handle_input(ui, hp, &hover, &mut response)
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

        self.draw(
            ui,
            painter,
            hp,
            hover,
            &response,
            current_input,
            &base_params,
        );

        if self.drawing.props.solve_continuously.is_some()
            && self.drawing.last_solve_error.is_some()
        {
            self.drawing.changed_in_ui();
            if self.drawing.last_solve_error.is_some() {
                ui.ctx().request_repaint();
            }
        }

        DrawResponse {}
    }
}

pub struct DrawResponse {}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn simplifications() {}
}
