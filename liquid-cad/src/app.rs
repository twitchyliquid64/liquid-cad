use detailer;
use drawing::{self, Feature, FeatureMeta};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    #[serde(skip)]
    drawing: drawing::Data,
    #[serde(skip)]
    handler: drawing::Handler,
    #[serde(skip)]
    tools: drawing::tools::Toolbar,
    #[serde(skip)]
    detailer_state: detailer::State,
}

impl Default for App {
    fn default() -> Self {
        let mut drawing = drawing::Data::default();

        let p1 = drawing
            .features
            .insert(Feature::Point(FeatureMeta::default(), -50., 0.));
        let p2 = drawing
            .features
            .insert(Feature::Point(FeatureMeta::default(), 0., 0.));
        let p3 = drawing
            .features
            .insert(Feature::Point(FeatureMeta::default(), 50., -50.));
        drawing
            .features
            .insert(Feature::LineSegment(FeatureMeta::default(), p2, p1));
        drawing
            .features
            .insert(Feature::LineSegment(FeatureMeta::default(), p3, p2));

        let tools = drawing::tools::Toolbar::default();
        let handler = drawing::Handler::default();
        let detailer_state = detailer::State::default();

        drawing.constraints.populate_cache();
        Self {
            drawing,
            handler,
            tools,
            detailer_state,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();

        if let Some(storage) = cc.storage {
            if let Some(saved) =
                eframe::get_value::<drawing::SerializedDrawing>(storage, eframe::APP_KEY)
            {
                if app.drawing.load(saved).err().is_some() {
                    println!("Failed to load diagram from storage");
                }
            } else if let Some(saved) = eframe::get_value::<(
                Vec<drawing::SerializedFeature>,
                Vec<drawing::SerializedConstraint>,
            )>(storage, eframe::APP_KEY)
            {
                // Legacy path
                if app
                    .drawing
                    .load(drawing::SerializedDrawing {
                        features: saved.0,
                        constraints: saved.1,
                        ..drawing::SerializedDrawing::default()
                    })
                    .err()
                    .is_some()
                {
                    println!("Failed to load diagram from storage (legacy)");
                }
            }
        }

        app
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.drawing.serialize());
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
                {
                    ui.menu_button("File", |ui| {
                        if ui.button("New").clicked() {
                            *self = App::default();
                        }
                        if ui.button("Save").clicked() {
                            self.save(frame.storage_mut().unwrap());
                        }
                        if ui.button("Reset egui state").clicked() {
                            ctx.memory_mut(|mem| *mem = Default::default());
                        }
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }
                #[cfg(target_arch = "wasm32")]
                {
                    ui.menu_button("File", |ui| {
                        if ui.button("New").clicked() {
                            *self = App::default();
                        }
                        if ui.button("Save").clicked() {
                            self.save(frame.storage_mut().unwrap());
                        }
                    });
                    ui.add_space(16.0);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            drawing::Widget::new(&mut self.drawing, &mut self.handler, &mut self.tools).show(ui);
        });

        detailer::Widget::new(
            &mut self.detailer_state,
            &mut self.drawing,
            &mut self.tools,
            &mut self.handler,
        )
        .show(ctx);
    }
}
