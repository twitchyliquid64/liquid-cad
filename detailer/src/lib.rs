use drawing::{tools::ToolResponse, Data, Feature, Handler};
use slotmap::DefaultKey as K;

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
    drawing: &'a mut Data<Feature>,
    handler: &'a mut Handler,
}

impl<'a> Widget<'a> {
    pub fn new(
        state: &'a mut State,
        drawing: &'a mut Data<Feature>,
        handler: &'a mut Handler,
    ) -> Self {
        Widget {
            state,
            drawing,
            handler,
        }
    }

    pub fn show(mut self, ctx: &egui::Context) {
        let mut window = egui::Window::new("Liquid CAD")
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
                    ui.add_space(4.);
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
        let mut commands: Vec<ToolResponse> = Vec::with_capacity(12);
        let selected: Vec<K> = self.drawing.selected_map.keys().map(|k| *k).collect();
        for k in selected {
            if let Some(v) = self.drawing.features.get_mut(k) {
                match v {
                    Feature::Point(x, y) => {
                        Widget::show_selection_entry_point(ui, &mut commands, &k, x, y)
                    }
                    Feature::LineSegment(p1, p2) => {
                        Widget::show_selection_entry_line(ui, &mut commands, &k)
                    }
                }
            }
        }

        use drawing::CommandHandler;
        for c in commands.drain(..) {
            self.handler.handle(self.drawing, c);
        }
    }

    fn show_selection_entry_point(
        ui: &mut egui::Ui,
        commands: &mut Vec<ToolResponse>,
        k: &K,
        px: &mut f32,
        py: &mut f32,
    ) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            use slotmap::Key;
            ui.add_sized(
                [r.x / 2., text_height],
                egui::Label::new(format!("Point {:?}", k.data().as_ffi())).wrap(false),
            );

            ui.add_sized([r.x / 6., text_height * 1.4], egui::DragValue::new(px));
            ui.add_sized([r.x / 6., text_height * 1.4], egui::DragValue::new(py));
            if ui.button("⊗").clicked() {
                commands.push(ToolResponse::Delete(*k));
            }
        });
    }

    fn show_selection_entry_line(ui: &mut egui::Ui, commands: &mut Vec<ToolResponse>, k: &K) {
        ui.horizontal(|ui| {
            let r = ui.available_size();
            let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

            use slotmap::Key;
            ui.add_sized(
                [r.x / 2., text_height],
                egui::Label::new(format!("Line {:?}", k.data().as_ffi())).wrap(false),
            );

            ui.allocate_exact_size([r.x / 3. + ui.spacing().item_spacing.x, text_height * 1.4].into(), egui::Sense::click());

            if ui.button("⊗").clicked() {
                commands.push(ToolResponse::Delete(*k));
            }
        });
    }

    fn show_system_tab(&mut self, ui: &egui::Ui) {}
}
