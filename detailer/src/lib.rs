use drawing::Handler;
use drawing::{handler::ToolResponse, tools, Data, Feature, FeatureKey, FeatureMeta};
use drawing::{Axis, Constraint, ConstraintKey, ConstraintMeta, DimensionDisplay};

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Tab {
    #[default]
    Selection,
    System,
}

#[derive(Debug, Default, Clone)]
pub struct State {
    tab: Tab,
}

pub struct Widget<'a> {
    state: &'a mut State,
    drawing: &'a mut Data,
    handler: &'a mut Handler,
    tools: &'a mut tools::Toolbar,
}

impl<'a> Widget<'a> {
    pub fn new(
        state: &'a mut State,
        drawing: &'a mut Data,
        tools: &'a mut tools::Toolbar,
        handler: &'a mut Handler,
    ) -> Self {
        Widget {
            state,
            drawing,
            handler,
            tools,
        }
    }

    pub fn show(mut self, ctx: &egui::Context) {
        let window = egui::Window::new("Liquid CAD")
            .id(egui::Id::new("detailer_window"))
            .resizable(false)
            .constrain(true)
            .collapsible(false)
            .title_bar(false)
            .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-4., 4.));

        window.show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                if ui
                    .selectable_label(self.state.tab == Tab::Selection, "Selection")
                    .clicked()
                {
                    self.state.tab = Tab::Selection
                };
                if ui
                    .selectable_label(self.state.tab == Tab::System, "System")
                    .clicked()
                {
                    self.state.tab = Tab::System
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.add_space(2.);
                    egui::warn_if_debug_build(ui);
                });
            });

            ui.separator();
            match self.state.tab {
                Tab::Selection => self.show_selection_tab(ui),
                Tab::System => self.show_system_tab(ui),
            }
        });
    }

    fn show_selection_tab(&mut self, ui: &mut egui::Ui) {
        let mut commands: Vec<ToolResponse> = Vec::with_capacity(4);
        let mut changed = false;
        let mut selected: Vec<FeatureKey> = self.drawing.selected_map.keys().map(|k| *k).collect();

        if let Some(ck) = self.drawing.selected_constraint {
            if let Some(c) = self.drawing.constraints.get(ck) {
                for fk in c.affecting_features() {
                    if !selected.contains(&fk) {
                        selected.push(fk);
                    }
                }
            }
        };

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
            ui.add_space(r.x / 2. - text_rect.width() - ui.spacing().item_spacing.x);

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
        axis: &mut Option<Axis>,
        _ref_pt: &mut DimensionDisplay,
        meta: &mut ConstraintMeta,
    ) {
        let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
        ui.horizontal(|ui| {
            let r = ui.available_size();

            let text_rect = ui.add(egui::Label::new("Length").wrap(false)).rect;
            ui.add_space(r.x / 2. - text_rect.width() - ui.spacing().item_spacing.x);

            let dv = ui.add_sized([50., text_height * 1.4], egui::DragValue::new(d));
            if meta.focus_to {
                meta.focus_to = false;
                dv.request_focus();
                ui.memory_mut(|mem| mem.request_focus(dv.id));
            } else {
                *changed |= dv.changed();
            }

            if *changed && *d < 0. {
                *d = 0.;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::ConstraintDelete(*k));
                }
            });
        });

        ui.horizontal(|ui| {
            let r = ui.available_size();

            match axis {
                Some(a) => {
                    let text_rect = ui.add(egui::Label::new("⏵ Cardinality").wrap(false)).rect;
                    ui.add_space(r.x / 2. - text_rect.width() - ui.spacing().item_spacing.x);

                    match a {
                        Axis::TopBottom => ui.label("+V"),
                        Axis::LeftRight => ui.label("+H"),
                    };

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        if ui.button("⊗").clicked() {
                            *axis = None;
                            *changed = true;
                        }
                    });
                }
                None => {
                    let r = ui.available_size();

                    let text_rect = ui
                        .add(egui::Label::new("⏵ Constrain cardinality").wrap(false))
                        .rect;
                    ui.add_space(r.x / 2. - text_rect.width() - ui.spacing().item_spacing.x);

                    if ui.button("+V").clicked() {
                        *axis = Some(Axis::TopBottom);
                        *changed = true;
                    }
                    if ui.button("+H").clicked() {
                        *axis = Some(Axis::LeftRight);
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
            ui.add_space(r.x / 2. - text_rect.width() - ui.spacing().item_spacing.x);

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

    fn show_selection_entry_point(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        changed: &mut bool,
        k: &FeatureKey,
        px: &mut f32,
        py: &mut f32,
        meta: &mut FeatureMeta,
    ) {
        // use egui_extras::{Size, StripBuilder};

        // StripBuilder::new(ui)
        //     .size(Size::relative(0.42)) // name cell
        //     .size(Size::relative(0.23)) // x cell
        //     .size(Size::relative(0.23)) // y cell
        //     .size(Size::remainder().at_least(25.0))
        //     .horizontal(|mut strip| {
        //         use slotmap::Key;
        //         strip.cell(|ui| {
        //             ui.label(format!("Point {:?}", k.data().as_ffi()));
        //         });
        //         strip.cell(|ui| {
        //             ui.add(egui::DragValue::new(px));
        //         });
        //         strip.cell(|ui| {
        //             ui.add(egui::DragValue::new(py));
        //         });
        //         strip.cell(|ui| {
        //             if ui.button("⊗").clicked() {
        //                 commands.push(ToolResponse::Delete(*k));
        //             }
        //         });
        //     });

        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            use slotmap::Key;
            let text_rect = ui
                .add(egui::Label::new(format!("Point {:?}", k.data())).wrap(false))
                .rect;
            if text_rect.width() < r.x / 4. - ui.spacing().item_spacing.x {
                ui.add_space(r.x / 4. - text_rect.width() - ui.spacing().item_spacing.x);
            }

            *changed |= ui
                .add_sized(
                    [r.x / 4., text_height * 1.4],
                    egui::Checkbox::new(&mut meta.construction, "🚧"),
                )
                .changed();

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
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            use slotmap::Key;
            let text_rect = ui
                .add(egui::Label::new(format!("Line {:?}", k.data())).wrap(false))
                .rect;
            if text_rect.width() < r.x / 4. - ui.spacing().item_spacing.x {
                ui.add_space(r.x / 4. - text_rect.width() - ui.spacing().item_spacing.x);
            }

            *changed |= ui
                .add_sized(
                    [r.x / 4., text_height * 1.4],
                    egui::Checkbox::new(&mut meta.construction, "🚧"),
                )
                .changed();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("⊗").clicked() {
                    commands.push(ToolResponse::Delete(*k));
                }
            });
        });
    }

    fn show_system_tab(&mut self, ui: &egui::Ui) {}
}
