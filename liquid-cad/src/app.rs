use drawing;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    drawing: drawing::Data<drawing::Feature>,

    #[serde(skip)]
    handler: drawing::Handler,
    #[serde(skip)]
    tools: drawing::tools::Toolbar,
}

impl Default for App {
    fn default() -> Self {
        let mut drawing = drawing::Data::default();
        drawing.features.insert(drawing::Feature::Point(-88., -88.));
        drawing.features.insert(drawing::Feature::Point(188., -88.));
        drawing.features.insert(drawing::Feature::Point(188., 188.));
        drawing.features.insert(drawing::Feature::Point(-88., 188.));

        let tools = drawing::tools::Toolbar::default();
        let handler = drawing::Handler::default();

        Self {
            drawing,
            handler,
            tools,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // if let Some(storage) = cc.storage {
        //     return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        // }

        Default::default()
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
                {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            _frame.close();
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            drawing::Widget::new(&mut self.drawing, &mut self.handler, &mut self.tools).show(ui);

            egui::warn_if_debug_build(
                &mut ui.child_ui(
                    ui.max_rect()
                        .split_left_right_at_x(ui.max_rect().max.x - 85.)
                        .1,
                    egui::Layout::default(),
                ),
            );
        });
    }
}
