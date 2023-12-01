use detailer;
use drawing::{self, Feature, FeatureMeta};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
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
        // drawing.features.insert(drawing::Feature::Point(-88., -88.));
        // drawing.features.insert(drawing::Feature::Point(188., -88.));
        // drawing.features.insert(drawing::Feature::Point(188., 188.));
        // drawing.features.insert(drawing::Feature::Point(-88., 188.));

        let p1 = drawing
            .features
            .insert(Feature::Point(FeatureMeta::default(), -50., 0.));
        let p2 = drawing
            .features
            .insert(Feature::Point(FeatureMeta::default(), 0., 0.));
        let p3 = drawing
            .features
            .insert(Feature::Point(FeatureMeta::default(), 50., -50.));
        // drawing
        //     .features
        //     .insert(Feature::LineSegment(FeatureMeta::default(), p1, p2));
        // drawing
        //     .features
        //     .insert(Feature::LineSegment(FeatureMeta::default(), p2, p3));
        drawing
            .features
            .insert(Feature::Arc(FeatureMeta::default(), p1, p2, p3));

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
                        if ui.button("New").clicked() {
                            *self = App::default();
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

                egui::widgets::global_dark_light_mode_buttons(ui);
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
