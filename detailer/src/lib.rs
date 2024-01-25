use drawing::Handler;
use drawing::CONSTRUCTION_IMG;
use drawing::{
    handler::ToolResponse, tools, Data, Feature, FeatureKey, FeatureMeta, SelectedElement,
};
use drawing::{Axis, Constraint, ConstraintKey, ConstraintMeta, DimensionDisplay};
use drawing::{Group, GroupType};

const FEATURE_NAME_WIDTH: f32 = 88.0;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Tab {
    #[default]
    Selection,
    Groups,
    General,
}

#[derive(Debug, Clone)]
pub struct State {
    tab: Tab,
    extrusion_amt: f64,
}

impl Default for State {
    fn default() -> Self {
        let tab = Tab::default();
        let extrusion_amt = 3.0;
        Self { tab, extrusion_amt }
    }
}

pub struct Widget<'a> {
    state: &'a mut State,
    drawing: &'a mut Data,
    handler: &'a mut Handler,
    tools: &'a mut tools::Toolbar,
    toasts: &'a mut egui_toast::Toasts,
}

impl<'a> Widget<'a> {
    pub fn new(
        state: &'a mut State,
        drawing: &'a mut Data,
        tools: &'a mut tools::Toolbar,
        handler: &'a mut Handler,
        toasts: &'a mut egui_toast::Toasts,
    ) -> Self {
        Widget {
            state,
            drawing,
            handler,
            tools,
            toasts,
        }
    }

    pub fn show<F>(mut self, ctx: &egui::Context, export_save: F)
    where
        F: FnOnce(&'static str, &'static str, Vec<u8>),
    {
        let window = egui::Window::new("Liquid CAD")
            .id(egui::Id::new("detailer_window"))
            .resizable(false)
            .constrain(true)
            .collapsible(false)
            .title_bar(false)
            .default_height(520.0)
            .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-4., 4.));

        window.show(ctx, |ui| {
            let (ctrl, one, two, three) = ui.input(|i| {
                (
                    i.modifiers.ctrl,
                    i.key_pressed(egui::Key::Num1),
                    i.key_pressed(egui::Key::Num2),
                    i.key_pressed(egui::Key::Num3),
                )
            });
            match (ctrl, one, two, three) {
                (true, true, _, _) => {
                    self.state.tab = Tab::Selection;
                }
                (true, _, true, _) => {
                    self.state.tab = Tab::Groups;
                }
                (true, _, _, true) => {
                    self.state.tab = Tab::General;
                }
                _ => {}
            }

            ui.horizontal_top(|ui| {
                if ui
                    .selectable_label(self.state.tab == Tab::Selection, "Selection")
                    .clicked()
                {
                    self.state.tab = Tab::Selection
                };
                if ui
                    .selectable_label(self.state.tab == Tab::Groups, "Groups")
                    .clicked()
                {
                    self.state.tab = Tab::Groups
                };
                if ui
                    .selectable_label(self.state.tab == Tab::General, "General")
                    .clicked()
                {
                    self.state.tab = Tab::General
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.add_space(2.);
                    egui::warn_if_debug_build(ui);
                });
            });

            ui.separator();
            match self.state.tab {
                Tab::Selection => self.show_selection_tab(ui),
                Tab::Groups => self.show_groups_tab(ui, export_save),
                Tab::General => self.show_general_tab(ui),
            }
        });
    }

    fn show_selection_tab(&mut self, ui: &mut egui::Ui) {
        let mut commands: Vec<ToolResponse> = Vec::with_capacity(4);
        let mut changed = false;
        let mut selected: Vec<FeatureKey> = self
            .drawing
            .selected_map
            .keys()
            .filter_map(|k| match k {
                SelectedElement::Feature(f) => Some(*f),
                _ => None,
            })
            .collect();

        for ck in self.drawing.selected_map.keys().filter_map(|e| {
            if let SelectedElement::Constraint(ck) = e {
                Some(*ck)
            } else {
                None
            }
        }) {
            if let Some(c) = self.drawing.constraints.get(ck) {
                for fk in c.affecting_features() {
                    if !selected.contains(&fk) {
                        selected.push(fk);
                    }
                }
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for k in selected {
                ui.push_id(k, |ui| {
                    match self.drawing.feature_mut(k) {
                        Some(Feature::Point(meta, x, y)) => Widget::show_selection_entry_point(
                            ui,
                            &mut commands,
                            &mut changed,
                            &k,
                            x,
                            y,
                            meta,
                        ),
                        Some(Feature::LineSegment(meta, _p1, _p2)) => {
                            Widget::show_selection_entry_line(
                                ui,
                                &mut commands,
                                &mut changed,
                                &k,
                                meta,
                            )
                        }
                        Some(Feature::Arc(meta, ..)) => Widget::show_selection_entry_arc(
                            ui,
                            &mut commands,
                            &mut changed,
                            &k,
                            meta,
                        ),
                        Some(Feature::Circle(meta, _p, radius)) => {
                            Widget::show_selection_entry_circle(
                                ui,
                                &mut commands,
                                &mut changed,
                                &k,
                                radius,
                                meta,
                            )
                        }
                        None => {}
                    }

                    let constraints = self.drawing.constraints_by_feature(&k);
                    if constraints.len() > 0 {
                        egui::CollapsingHeader::new("Constraints")
                            .default_open(true)
                            .show(ui, |ui| {
                                for ck in constraints {
                                    ui.push_id(k, |ui| match self.drawing.constraint_mut(ck) {
                                        Some(Constraint::Fixed(_, _, x, y)) => {
                                            Widget::show_constraint_fixed(
                                                ui,
                                                &mut commands,
                                                &mut changed,
                                                &ck,
                                                x,
                                                y,
                                            )
                                        }
                                        Some(Constraint::LineLength(meta, _, d, axis, dd)) => {
                                            Widget::show_constraint_line_length(
                                                ui,
                                                &mut commands,
                                                &mut changed,
                                                &ck,
                                                d,
                                                axis,
                                                dd,
                                                meta,
                                            )
                                        }
                                        Some(Constraint::LineAlongCardinal(
                                            _,
                                            _,
                                            is_horizontal,
                                        )) => Widget::show_constraint_line_cardinal_align(
                                            ui,
                                            &mut commands,
                                            &mut changed,
                                            &ck,
                                            is_horizontal,
                                        ),
                                        Some(Constraint::PointLerpLine(meta, _, _, amt)) => {
                                            Widget::show_constraint_line_lerp(
                                                ui,
                                                &mut commands,
                                                &mut changed,
                                                &ck,
                                                amt,
                                                meta,
                                            )
                                        }
                                        Some(Constraint::LineLengthsEqual(
                                            _meta,
                                            _k1,
                                            _k2,
                                            ratio,
                                            ..,
                                        )) => Widget::show_constraint_line_equal(
                                            ui,
                                            &mut commands,
                                            ratio,
                                            &mut changed,
                                            &ck,
                                        ),
                                        Some(Constraint::LinesParallel(..)) => {
                                            Widget::show_constraint_lines_parallel(
                                                ui,
                                                &mut commands,
                                                &mut changed,
                                                &ck,
                                            )
                                        }
                                        Some(Constraint::CircleRadius(meta, _center, amt, ..)) => {
                                            Widget::show_constraint_circle_radius(
                                                ui,
                                                &mut commands,
                                                &mut changed,
                                                &ck,
                                                amt,
                                                meta,
                                            )
                                        }
                                        Some(Constraint::CircleRadiusEqual(
                                            _meta,
                                            _fk1,
                                            _fk2,
                                            ratio,
                                        )) => Widget::show_constraint_circle_radius_equal(
                                            ui,
                                            &mut commands,
                                            ratio,
                                            &mut changed,
                                            &ck,
                                        ),
                                        Some(Constraint::LineAngle(
                                            meta,
                                            _line,
                                            angle_radians,
                                            ..,
                                        )) => Widget::show_constraint_line_angle(
                                            ui,
                                            &mut commands,
                                            &mut changed,
                                            &ck,
                                            angle_radians,
                                            meta,
                                        ),
                                        None => {}
                                    });
                                }
                            });
                    }
                });
            }
        });

        for c in commands.drain(..) {
            self.handler.handle(self.drawing, self.tools, c);
        }
        if changed {
            self.drawing.changed_in_ui();
        }
    }

    fn show_constraint_fixed(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &ConstraintKey,
        px: &mut f32,
        py: &mut f32,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            let text_rect = ui.add(egui::Label::new("Fixed").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            *changed |= ui
                .add_sized([50., text_height * 1.4], egui::DragValue::new(px))
                .changed();
            *changed |= ui
                .add_sized([50., text_height * 1.4], egui::DragValue::new(py))
                .changed();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });
    }

    fn show_constraint_line_length(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &ConstraintKey,
        d: &mut f32,
        aa_info: &mut Option<(Axis, bool)>,
        ref_pt: &mut DimensionDisplay,
        _meta: &mut ConstraintMeta,
    ) {
        let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
        ui.horizontal(|ui| {
            let r = ui.available_size();

            let text_rect = ui.add(egui::Label::new("Length").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            let dv = ui.add_sized([50., text_height * 1.4], egui::DragValue::new(d));
            *changed |= dv.changed();

            if *changed && *d < 0. {
                *d = 0.;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
                if ui.button("V🔃").clicked() {
                    ref_pt.next_variant();
                    *changed = true;
                }
            });
        });

        ui.horizontal(|ui| {
            let r = ui.available_size();

            match aa_info {
                Some((a, is_neg)) => {
                    let text_rect = ui.add(egui::Label::new("⏵ Cardinality").wrap(false)).rect;
                    ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

                    let text_rect = match (a, &is_neg) {
                        (Axis::TopBottom, false) => ui.label("+V"),
                        (Axis::TopBottom, true) => ui.label("-V"),
                        (Axis::LeftRight, false) => ui.label("+H"),
                        (Axis::LeftRight, true) => ui.label("-H"),
                    }
                    .rect;
                    ui.add_space(
                        ui.spacing().interact_size.x + (ui.spacing().item_spacing.x * 7.0 / 6.0)
                            - text_rect.width(),
                    );

                    if ui.button("invert").clicked() {
                        *is_neg = !*is_neg;
                        *changed = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        if ui.button("⊗").clicked() {
                            *aa_info = None;
                            *changed = true;
                        }
                    });
                }
                None => {
                    let r = ui.available_size();

                    let text_rect = ui
                        .add(egui::Label::new("⏵ Constrain cardinality").wrap(false))
                        .rect;
                    ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

                    if ui.button("-V").clicked() {
                        *aa_info = Some((Axis::TopBottom, true));
                        *changed = true;
                    }
                    if ui.button("+V").clicked() {
                        *aa_info = Some((Axis::TopBottom, false));
                        *changed = true;
                    }
                    if ui.button("-H").clicked() {
                        *aa_info = Some((Axis::LeftRight, true));
                        *changed = true;
                    }
                    if ui.button("+H").clicked() {
                        *aa_info = Some((Axis::LeftRight, false));
                        *changed = true;
                    }
                }
            };
        });
    }

    fn show_constraint_line_cardinal_align(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &ConstraintKey,
        axis: &mut Axis,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            let text_rect = ui
                .add(
                    egui::Label::new(if *axis == Axis::LeftRight {
                        "Horizontal"
                    } else {
                        "Vertical"
                    })
                    .wrap(false),
                )
                .rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            let resp = ui.add_sized(
                [100. + ui.spacing().item_spacing.x, text_height * 1.4],
                egui::Button::new("swap direction"),
            );

            if resp.clicked() {
                *changed |= true;
                axis.swap();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });
    }

    fn show_constraint_line_lerp(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &ConstraintKey,
        amt: &mut f32,
        _meta: &mut ConstraintMeta,
    ) {
        let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
        ui.horizontal(|ui| {
            let r = ui.available_size();

            let text_rect = ui.add(egui::Label::new("Point lerp").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            let dv = ui.add_sized(
                [50., text_height * 1.4],
                egui::DragValue::new(amt)
                    .clamp_range(0.0..=1.0)
                    .speed(0.005),
            );
            *changed |= dv.changed();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });
    }

    fn show_constraint_line_equal(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        ratio: &mut Option<f32>,
        changed: &mut bool,
        k: &ConstraintKey,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            let text_rect = ui.add(egui::Label::new("Equal length").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            if ratio.is_none() {
                let resp = ui.add_sized(
                    [100. + ui.spacing().item_spacing.x, text_height * 1.4],
                    egui::Button::new("set multiplier"),
                );

                if resp.clicked() {
                    *changed |= true;
                    *ratio = Some(0.5);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });

        if let Some(m) = ratio {
            ui.horizontal(|ui| {
                let r = ui.available_size();
                let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

                let text_rect = ui.add(egui::Label::new("⏵ Multiplier").wrap(false)).rect;
                ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

                let dv = ui.add_sized(
                    [50., text_height * 1.4],
                    egui::DragValue::new(m).clamp_range(0.05..=20.0).speed(0.01),
                );
                *changed |= dv.changed();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button("⊗").clicked() {
                        commands.push(ToolResponse::ConstraintLinesEqualRemoveMultiplier(*k));
                    }
                });
            });
        }
    }

    fn show_constraint_lines_parallel(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        _changed: &mut bool,
        k: &ConstraintKey,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();

            let text_rect = ui.add(egui::Label::new("Parallel").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });
    }

    fn show_constraint_circle_radius(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &ConstraintKey,
        amt: &mut f32,
        _meta: &mut ConstraintMeta,
    ) {
        let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
        ui.horizontal(|ui| {
            let r = ui.available_size();

            let text_rect = ui.add(egui::Label::new("Radius").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            let dv = ui.add_sized(
                [50., text_height * 1.4],
                egui::DragValue::new(amt)
                    .clamp_range(0.0..=200.0)
                    .speed(0.05),
            );
            *changed |= dv.changed();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });
    }

    fn show_constraint_circle_radius_equal(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        ratio: &mut Option<f32>,
        changed: &mut bool,
        k: &ConstraintKey,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            let text_rect = ui.add(egui::Label::new("Equal radius").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            if ratio.is_none() {
                let resp = ui.add_sized(
                    [100. + ui.spacing().item_spacing.x, text_height * 1.4],
                    egui::Button::new("set multiplier"),
                );

                if resp.clicked() {
                    *changed |= true;
                    *ratio = Some(0.5);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });

        if let Some(m) = ratio {
            ui.horizontal(|ui| {
                let r = ui.available_size();
                let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

                let text_rect = ui.add(egui::Label::new("⏵ Multiplier").wrap(false)).rect;
                ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

                let dv = ui.add_sized(
                    [50., text_height * 1.4],
                    egui::DragValue::new(m).clamp_range(0.05..=20.0).speed(0.01),
                );
                *changed |= dv.changed();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button("⊗").clicked() {
                        commands.push(ToolResponse::ConstraintRadiusEqualRemoveMultiplier(*k));
                    }
                });
            });
        }
    }

    fn show_constraint_line_angle(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &ConstraintKey,
        amt: &mut f32,
        _meta: &mut ConstraintMeta,
    ) {
        let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
        ui.horizontal(|ui| {
            let r = ui.available_size();

            let text_rect = ui.add(egui::Label::new("Line angle").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - 3.0 * ui.spacing().item_spacing.x);

            let mut degrees = (*amt + (0.5 * std::f32::consts::PI)).to_degrees();

            let dv = ui.add_sized(
                [50., text_height * 1.4],
                egui::DragValue::new(&mut degrees)
                    .clamp_range(-360.0..=360.0)
                    .speed(0.1)
                    .suffix("°"),
            );

            if dv.changed() {
                *amt = degrees.to_radians() - (0.5 * std::f32::consts::PI);
                *changed |= true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });
    }

    fn show_selection_entry_point(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &FeatureKey,
        px: &mut f32,
        py: &mut f32,
        meta: &mut FeatureMeta,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            use slotmap::Key;
            ui.add(
                egui::Label::new(format!("Point {:?}", k.data()))
                    .wrap(false)
                    .truncate(true),
            );
            if r.x - ui.available_width() < FEATURE_NAME_WIDTH {
                ui.add_space(FEATURE_NAME_WIDTH - (r.x - ui.available_width()));
            }

            *changed |= ui
                .add(egui::Checkbox::without_text(&mut meta.construction))
                .changed();
            ui.add(egui::Image::new(CONSTRUCTION_IMG).rounding(5.0));

            if ui.available_width() > r.x / 2. - ui.spacing().item_spacing.x {
                ui.add_space(ui.available_width() - r.x / 2. - ui.spacing().item_spacing.x);
            }

            *changed |= ui
                .add_sized([50., text_height * 1.4], egui::DragValue::new(px))
                .changed();
            *changed |= ui
                .add_sized([50., text_height * 1.4], egui::DragValue::new(py))
                .changed();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::Delete(*k));
                }
            });
        });
    }

    fn show_selection_entry_line(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &FeatureKey,
        meta: &mut FeatureMeta,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();

            use slotmap::Key;
            ui.add(
                egui::Label::new(format!("Line {:?}", k.data()))
                    .wrap(false)
                    .truncate(true),
            );
            if r.x - ui.available_width() < FEATURE_NAME_WIDTH {
                ui.add_space(FEATURE_NAME_WIDTH - (r.x - ui.available_width()));
            }

            *changed |= ui
                .add(egui::Checkbox::without_text(&mut meta.construction))
                .changed();
            ui.add(egui::Image::new(CONSTRUCTION_IMG).rounding(5.0));

            if ui.available_width() > r.x / 2. - ui.spacing().item_spacing.x {
                ui.add_space(ui.available_width() - r.x / 2. - ui.spacing().item_spacing.x);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::Delete(*k));
                }
            });
        });
    }

    fn show_selection_entry_arc(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &FeatureKey,
        meta: &mut FeatureMeta,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();

            use slotmap::Key;
            ui.add(
                egui::Label::new(format!("Arc {:?}", k.data()))
                    .wrap(false)
                    .truncate(true),
            );
            if r.x - ui.available_width() < FEATURE_NAME_WIDTH {
                ui.add_space(FEATURE_NAME_WIDTH - (r.x - ui.available_width()));
            }

            *changed |= ui
                .add(egui::Checkbox::without_text(&mut meta.construction))
                .changed();
            ui.add(egui::Image::new(CONSTRUCTION_IMG).rounding(5.0));

            if ui.available_width() > r.x / 2. - ui.spacing().item_spacing.x {
                ui.add_space(ui.available_width() - r.x / 2. - ui.spacing().item_spacing.x);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::Delete(*k));
                }
            });
        });
    }

    fn show_selection_entry_circle(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &FeatureKey,
        radius: &mut f32,
        meta: &mut FeatureMeta,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            use slotmap::Key;
            ui.add(
                egui::Label::new(format!("Circle {:?}", k.data()))
                    .wrap(false)
                    .truncate(true),
            );
            if r.x - ui.available_width() < FEATURE_NAME_WIDTH {
                ui.add_space(FEATURE_NAME_WIDTH - (r.x - ui.available_width()));
            }

            *changed |= ui
                .add(egui::Checkbox::without_text(&mut meta.construction))
                .changed();
            ui.add(egui::Image::new(CONSTRUCTION_IMG).rounding(5.0));

            if ui.available_width() > r.x / 2. - ui.spacing().item_spacing.x {
                ui.add_space(ui.available_width() - r.x / 2. - ui.spacing().item_spacing.x);
            }

            *changed |= ui
                .add_sized(
                    [50., text_height * 1.4],
                    egui::DragValue::new(radius)
                        .clamp_range(0.0..=5000.0)
                        .speed(0.05),
                )
                .changed();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::Delete(*k));
                }
            });
        });
    }

    fn show_groups_tab<F>(&mut self, ui: &mut egui::Ui, export_save: F)
    where
        F: FnOnce(&'static str, &'static str, Vec<u8>),
    {
        let mut commands: Vec<ToolResponse> = Vec::with_capacity(4);
        let mut boundary_group_set: Option<usize> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label("Groups are a collection of drawing elements that form a path. Use them to label collections of elements as interior geometry, boundary geometry, etc.");
            ui.add_space(10.0);

            for (i, group) in self.drawing.groups.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    let id = ui.make_persistent_id("header_group");
                    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                        .show_header(ui, |ui| {
                            ui.horizontal(|ui| {
                                let r = ui.available_size();

                                let name_input = egui::widgets::TextEdit::singleline(&mut group.name)
                                    .hint_text("Group name")
                                    .desired_width(r.x / 2.0)
                                    .clip_text(true);
                                ui.add(name_input);

                                egui::ComboBox::from_id_source("type combo")
                                    .selected_text(format!("{:?}", group.typ))
                                    .show_ui(ui, |ui| {
                                        ui.style_mut().wrap = Some(false);
                                        ui.set_min_width(60.0);
                                        ui.selectable_value(&mut group.typ, GroupType::Hole, "Hole");
                                        ui.selectable_value(&mut group.typ, GroupType::Extrude, "Extrude");
                                        if ui.selectable_value(&mut group.typ, GroupType::Boundary, "Boundary").changed() {
                                            boundary_group_set = Some(i);
                                        };
                                    }
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                    if ui.button("⊗").clicked() {
                                        commands.push(ToolResponse::DeleteGroup(i));
                                    }
                                });
                            });
                        })
                        .body(|ui| {
                            ui.horizontal(|ui| {
                                let r = ui.available_size();
                                let text_rect = ui.add(egui::Label::new(format!("{} features", group.features.len())).wrap(false)).rect;

                                if text_rect.width() < r.x / 2. {
                                    ui.add_space(r.x / 2. - text_rect.width());
                                }
                                if ui.button("Select").clicked() {
                                    self.drawing.selected_map = std::collections::HashMap::from_iter(
                                        group.features.iter().enumerate().map(|(i, fk)| (SelectedElement::Feature(*fk), i))
                                    );
                                };
                                if ui.add_enabled(group.features.len() > 0, egui::Button::new("Clear")).clicked() {
                                    group.features.clear();
                                };
                            });

                            ui.horizontal(|ui| {
                                if ui.button("+ Add from selection").clicked() {
                                    for fk in self.drawing.selected_map.keys().filter_map(|e| if let SelectedElement::Feature(f) = e { Some(f) } else { None }) {
                                        if let Some(f) = self.drawing.features.get(*fk) {
                                            if f.is_point() || f.is_construction() {
                                                continue;
                                            }
                                        }
                                        if group.features.iter().position(|k| k == fk).is_none() {
                                            group.features.push(*fk);
                                        }
                                    }
                                };
                            });
                        });
                    });

                ui.add_space(6.0);
            }

            ui.add_space(6.0);
            if ui.button("New +").clicked() {
                let g_len = self.drawing.groups.len();
                self.drawing.groups.push(Group{
                    name: "Unnamed group".into(),
                    typ: if g_len == 0 {
                        GroupType::Boundary
                    } else {
                        GroupType::Hole
                    },
                    ..Group::default()
                });
            }

            ui.add_space(12.0);
            ui.label("Export");
            ui.separator();
            ui.add_space(2.0);
            ui.add(egui::Slider::new(&mut self.drawing.props.flatten_tolerance, 0.0001..=5.0)
                    .text("Flatten tolerance").suffix("mm").logarithmic(true));

            if let Some(err) = self.drawing.last_solve_error {
                ui.add(egui::Label::new(egui::RichText::new(format!("⚠ Solver is inconsistent!! avg err: {:.3}mm", err))
                .color(ui.visuals().warn_fg_color)));
                ui.add_space(5.0);
            }

            ui.add_space(5.0);

            use std::cell::OnceCell;
            let mut export_fn = OnceCell::new();
            export_fn.set(export_save).ok();

            ui.horizontal(|ui| {
                let r = ui.available_size();
                let text_rect = ui.add(egui::Label::new("OpenSCAD Polygon")).rect;
                if text_rect.width() < r.x / 2. {
                    ui.add_space(r.x / 2. - text_rect.width());
                }

                if ui.add_enabled(self.drawing.groups.len() > 0, egui::Button::new("Clipboard 📋")).clicked() {
                    if let Ok(t) = self.drawing.serialize_openscad(self.drawing.props.flatten_tolerance) {
                        ui.ctx().output_mut(|o| o.copied_text = t);
                        self.toasts.add(egui_toast::Toast {
                            text: "OpenSCAD code copied to clipboard!".into(),
                            kind: egui_toast::ToastKind::Info,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(3.5)
                                .show_progress(true)
                        });
                    } else {
                        self.toasts.add(egui_toast::Toast {
                            text: "Export failed!".into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(4.0)
                                .show_progress(true)
                        });
                    }
                }
                if ui.add_enabled(self.drawing.groups.len() > 0, egui::Button::new("File 📥")).clicked() {
                    if let Ok(t) = self.drawing.serialize_openscad(self.drawing.props.flatten_tolerance) {
                        export_fn.take().map(|f| f("OpenSCAD", "scad", t.into()));
                    } else {
                        self.toasts.add(egui_toast::Toast {
                            text: "Export failed!".into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(4.0)
                                .show_progress(true)
                        });
                    }
                }
            });

            ui.horizontal(|ui| {
                let r = ui.available_size();
                let text_rect = ui.add(egui::Label::new("DXF")).rect;
                if text_rect.width() < r.x / 2. {
                    ui.add_space(r.x / 2. - text_rect.width());
                }

                if ui.add_enabled(self.drawing.groups.len() > 0, egui::Button::new("Clipboard 📋")).clicked() {
                    if let Ok(t) = self.drawing.serialize_dxf(self.drawing.props.flatten_tolerance) {
                        ui.ctx().output_mut(|o| o.copied_text = t);
                        self.toasts.add(egui_toast::Toast {
                            text: "DXF code copied to clipboard!".into(),
                            kind: egui_toast::ToastKind::Info,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(3.5)
                                .show_progress(true)
                        });
                    } else {
                        self.toasts.add(egui_toast::Toast {
                            text: "Export failed!".into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(4.0)
                                .show_progress(true)
                        });
                    }
                }
                if ui.add_enabled(self.drawing.groups.len() > 0, egui::Button::new("File 📥")).clicked() {
                    if let Ok(t) = self.drawing.serialize_dxf(self.drawing.props.flatten_tolerance) {
                        export_fn.take().map(|f| f("AutoCAD DXF", "dxf", t.into()));
                    } else {
                        self.toasts.add(egui_toast::Toast {
                            text: "Export failed!".into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(4.0)
                                .show_progress(true)
                        });
                    }
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let r = ui.available_size();
                let text_rect = ui.add(egui::Label::new("3D extrusion")).rect;
                if text_rect.width() < r.x / 2. {
                    ui.add_space(r.x / 2. - text_rect.width());
                }

                ui.add(egui::DragValue::new(&mut self.state.extrusion_amt)
                        .speed(0.1).suffix("mm"));

                if self.state.extrusion_amt < 0.1 {
                    self.state.extrusion_amt = 0.1;
                }

                if ui.add_enabled(self.drawing.groups.len() > 0, egui::Button::new("STL 📥")).clicked() {
                    match self.drawing.as_solid(self.state.extrusion_amt) {
                        Ok(solid) => {
                            use drawing::l::three_d::*;
                            export_fn.take().map(|f| f("STL", "stl", solid_to_stl(solid, self.drawing.props.flatten_tolerance)));
                        },
                        Err(err) => {
                            self.toasts.add(egui_toast::Toast {
                                text: format!("Export failed!\n\nErr: {:?}", err).into(),
                                kind: egui_toast::ToastKind::Error,
                                options: egui_toast::ToastOptions::default()
                                    .duration_in_seconds(4.0)
                                    .show_progress(true)
                            });
                        }
                    }
                }
                if ui.add_enabled(self.drawing.groups.len() > 0, egui::Button::new("OBJ 📥")).clicked() {
                    match self.drawing.as_solid(self.state.extrusion_amt) {
                        Ok(solid) => {
                            use drawing::l::three_d::*;
                            export_fn.take().map(|f| f("OBJ", "obj", solid_to_obj(solid, self.drawing.props.flatten_tolerance)));
                        },
                        Err(err) => {
                            self.toasts.add(egui_toast::Toast {
                                text: format!("Export failed!\n\nErr: {:?}", err).into(),
                                kind: egui_toast::ToastKind::Error,
                                options: egui_toast::ToastOptions::default()
                                    .duration_in_seconds(4.0)
                                    .show_progress(true)
                            });
                        }
                    }
                }
            });
        });

        if let Some(idx) = boundary_group_set {
            for (i, g) in self.drawing.groups.iter_mut().enumerate() {
                if i != idx && g.typ == GroupType::Boundary {
                    g.typ = GroupType::Hole;
                }
            }
        }
        for c in commands.drain(..) {
            self.handler.handle(self.drawing, self.tools, c);
        }
    }

    fn show_general_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.drawing.props.name)
                .hint_text("Untitled drawing")
                .desired_width(f32::INFINITY),
        );

        ui.add_space(10.0);
        ui.label("General settings");

        let mut cont_solve = self.drawing.props.solve_continuously.is_some();
        if ui
            .add(egui::Checkbox::new(
                &mut cont_solve,
                "Solve continuously when inconsistent",
            ))
            .changed()
        {
            self.drawing.props.solve_continuously = if cont_solve { Some(()) } else { None };
            self.drawing.changed_in_ui();
        }

        if ui
            .add(
                egui::Slider::new(&mut self.drawing.props.solver_stop_err, 0.1..=0.00001)
                    .text("Solver desired accuracy")
                    .suffix("mm")
                    .min_decimals(7)
                    .logarithmic(true),
            )
            .changed()
        {
            self.drawing.changed_in_ui();
        };
        ui.add(
            egui::Slider::new(&mut self.drawing.props.flatten_tolerance, 0.0001..=5.0)
                .text("Flatten tolerance")
                .suffix("mm")
                .min_decimals(7)
                .logarithmic(true),
        );
    }
}
