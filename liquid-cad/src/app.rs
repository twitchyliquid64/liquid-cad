use detailer;
use drawing;
use helper;
use std::sync::mpsc::{channel, Receiver, Sender};

#[cfg(target_arch = "wasm32")]
fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}

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
    #[serde(skip)]
    helper_state: helper::State,
    #[serde(skip)]
    toasts: egui_toast::Toasts,

    show_help: bool,

    #[serde(skip)]
    last_path: Option<std::path::PathBuf>,
    #[serde(skip)]
    wasm_open_channel: (Sender<(String, String)>, Receiver<(String, String)>),
}

impl Default for App {
    fn default() -> Self {
        let drawing = drawing::Data::default();
        let tools = drawing::tools::Toolbar::default();
        let handler = drawing::Handler::default();
        let detailer_state = detailer::State::default();
        let helper_state = helper::State::default();
        let toasts = egui_toast::Toasts::new()
            .anchor(egui::Align2::RIGHT_BOTTOM, (-10.0, -10.0)) // 10 units from the bottom right corner
            .direction(egui::Direction::BottomUp);

        let last_path = None;
        let wasm_open_channel = channel();
        let show_help = true;

        Self {
            drawing,
            handler,
            tools,
            detailer_state,
            helper_state,
            toasts,
            show_help,
            last_path,
            wasm_open_channel,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut app = Self::default();

        if let Some(storage) = cc.storage {
            if let Some(saved) =
                eframe::get_value::<drawing::SerializedDrawing>(storage, eframe::APP_KEY)
            {
                if app.drawing.load(saved).err().is_some() {
                    println!("Failed to load diagram from storage");
                } else {
                    app.show_help = false;
                }
            } else {
                println!("nothing read from storage");
            }
        }

        app
    }

    fn export_str_as(&mut self, type_name: &'static str, ext_name: &'static str, data: Vec<u8>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use rfd::FileDialog;
            let file = FileDialog::new()
                .add_filter(type_name, &[ext_name])
                .add_filter("text", &["txt"])
                .set_file_name(format!("export.{}", ext_name))
                .save_file();

            if let Some(path) = file {
                match std::fs::write(path.clone(), data) {
                    Ok(_) => {}
                    Err(e) => {
                        self.toasts.add(egui_toast::Toast {
                            text: format!("Save failed!\n{:?}", e).into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(5.0)
                                .show_progress(true),
                        });
                    }
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let task = rfd::AsyncFileDialog::new()
                .set_file_name(format!("export.{}", ext_name))
                .save_file();
            execute(async move {
                let file = task.await;
                if let Some(file) = file {
                    let _ = file.write(data.as_slice()).await;
                }
            });
        }
    }

    pub fn save_as(&mut self) {
        let ser_config = ron::ser::PrettyConfig::new()
            .depth_limit(4)
            .indentor("\t".to_owned());

        let file_name: String = match &self.last_path {
            Some(pb) => pb.file_name().unwrap().to_str().unwrap().to_owned(),
            None => "untitled.lcad".to_owned(),
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            use rfd::FileDialog;
            let file = FileDialog::new()
                .add_filter("liquid cad", &["lcad"])
                .add_filter("text", &["txt"])
                .set_file_name(file_name)
                .save_file();

            if let Some(path) = file {
                let sd = &self.drawing.serialize();

                match std::fs::write(
                    path.clone(),
                    ron::ser::to_string_pretty(sd, ser_config)
                        .unwrap()
                        .as_bytes(),
                ) {
                    Ok(_) => {
                        self.last_path = Some(path);
                    }
                    Err(e) => {
                        self.toasts.add(egui_toast::Toast {
                            text: format!("Save failed!\n{:?}", e).into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(5.0)
                                .show_progress(true),
                        });
                    }
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let sd = (&self.drawing.serialize()).clone();
            let task = rfd::AsyncFileDialog::new()
                .set_file_name(file_name)
                .save_file();
            execute(async move {
                let file = task.await;
                if let Some(file) = file {
                    let _ = file
                        .write(
                            ron::ser::to_string_pretty(&sd, ser_config)
                                .unwrap()
                                .as_bytes(),
                        )
                        .await;
                }
            });
        }
    }

    pub fn open_from(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use rfd::FileDialog;
            let file = FileDialog::new()
                .add_filter("liquid cad", &["lcad"])
                .add_filter("text", &["txt"])
                .pick_file();

            if let Some(path) = file {
                match std::fs::read(path.clone()) {
                    Ok(b) => match ron::de::from_bytes(&b) {
                        Ok(d) => {
                            if let Some(e) = self.drawing.load(d).err() {
                                self.toasts.add(egui_toast::Toast {
                                    text: format!("Load failed: {:?}", e).into(),
                                    kind: egui_toast::ToastKind::Error,
                                    options: egui_toast::ToastOptions::default()
                                        .duration_in_seconds(5.0)
                                        .show_progress(true),
                                });
                            } else {
                                self.last_path = Some(path);
                            }
                        }

                        Err(e) => {
                            self.toasts.add(egui_toast::Toast {
                                text: format!("Deserialize failed: {:?}", e).into(),
                                kind: egui_toast::ToastKind::Error,
                                options: egui_toast::ToastOptions::default()
                                    .duration_in_seconds(5.0)
                                    .show_progress(true),
                            });
                        }
                    },

                    Err(e) => {
                        self.toasts.add(egui_toast::Toast {
                            text: format!("Read failed: {:?}", e).into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(5.0)
                                .show_progress(true),
                        });
                    }
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let sender = self.wasm_open_channel.0.clone();
            let task = rfd::AsyncFileDialog::new().pick_file();
            execute(async move {
                let file = task.await;
                if let Some(file) = file {
                    let text = file.read().await;
                    let _ =
                        sender.send((file.file_name(), String::from_utf8_lossy(&text).to_string()));
                }
            });
        }
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.drawing.serialize());
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let (mut center, mut zoom) = (false, false);
        // type name, extension, data
        let mut pending_export: Option<(&'static str, &'static str, Vec<u8>)> = None;

        #[cfg(target_arch = "wasm32")]
        if let Ok((fname, contents)) = self.wasm_open_channel.1.try_recv() {
            match ron::de::from_str(&contents) {
                Ok(d) => {
                    if let Some(e) = self.drawing.load(d).err() {
                        self.toasts.add(egui_toast::Toast {
                            text: format!("Load failed: {:?}", e).into(),
                            kind: egui_toast::ToastKind::Error,
                            options: egui_toast::ToastOptions::default()
                                .duration_in_seconds(5.0)
                                .show_progress(true),
                        });
                    } else {
                        self.last_path = Some(fname.into());
                    }
                }

                Err(e) => {
                    self.toasts.add(egui_toast::Toast {
                        text: format!("Deserialize failed: {:?}", e).into(),
                        kind: egui_toast::ToastKind::Error,
                        options: egui_toast::ToastOptions::default()
                            .duration_in_seconds(5.0)
                            .show_progress(true),
                    });
                }
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
                {
                    ui.menu_button("File", |ui| {
                        if ui.button("New").clicked() {
                            *self = App::default();
                        }
                        if ui.button("Open").clicked() {
                            self.open_from();
                        }
                        if ui.button("Save As").clicked() {
                            self.save_as();
                        }
                        if ui.button("Quick save").clicked() {
                            self.save(frame.storage_mut().unwrap());
                        }
                        ui.separator();
                        if ui.button("Reset egui state").clicked() {
                            ctx.memory_mut(|mem| *mem = Default::default());
                        }
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                }
                #[cfg(target_arch = "wasm32")]
                {
                    ui.menu_button("File", |ui| {
                        if ui.button("New").clicked() {
                            *self = App::default();
                        }
                        if ui.button("Open").clicked() {
                            self.open_from();
                        }
                        if ui.button("Save as").clicked() {
                            self.save_as();
                        }
                        if ui.button("Quick save").clicked() {
                            self.save(frame.storage_mut().unwrap());
                        }
                        ui.separator();
                        if ui.button("Reset egui state").clicked() {
                            ctx.memory_mut(|mem| *mem = Default::default());
                        }
                    });
                }
                ui.add_space(8.0);

                ui.menu_button("Drawing", |ui| {
                    if ui.button("Center").clicked() {
                        center = true;
                    }
                    if ui.button("Center & zoom").clicked() {
                        center = true;
                        zoom = true;
                    }
                    ui.separator();
                    if ui.button("Solve step").clicked() {
                        self.drawing.changed_in_ui();
                    }
                    // if ui.button("Bruteforce solve").clicked() {
                    //     self.drawing.bruteforce_solve();
                    // }
                });
                ui.add_space(8.0);

                ui.menu_button("Selection", |ui| {
                    if ui.button("Clear   (Esc)").clicked() {
                        self.drawing.selection_clear();
                    }
                    if ui.button("Select all   (Ctrl-A)").clicked() {
                        self.drawing.select_all();
                    }
                    ui.menu_button("Select feature...", |ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::Image::new(drawing::CONSTRUCTION_IMG).rounding(5.0));
                            ui.checkbox(
                                &mut self.drawing.select_action_inc_construction,
                                "include construction features",
                            );
                        });
                        ui.separator();
                        use slotmap::Key;
                        if ui.button("Points").clicked() {
                            self.drawing.select_type(&drawing::Feature::Point(
                                drawing::FeatureMeta::default(),
                                0.,
                                0.,
                            ));
                        }
                        if ui.button("Lines").clicked() {
                            self.drawing.select_type(&drawing::Feature::LineSegment(
                                drawing::FeatureMeta::default(),
                                drawing::FeatureKey::null(),
                                drawing::FeatureKey::null(),
                            ));
                        }
                        if ui.button("Circles").clicked() {
                            self.drawing.select_type(&drawing::Feature::Circle(
                                drawing::FeatureMeta::default(),
                                drawing::FeatureKey::null(),
                                0.,
                            ));
                        }
                        if ui.button("Arcs").clicked() {
                            self.drawing.select_type(&drawing::Feature::Arc(
                                drawing::FeatureMeta::default(),
                                drawing::FeatureKey::null(),
                                drawing::FeatureKey::null(),
                                drawing::FeatureKey::null(),
                            ));
                        }
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.checkbox(&mut self.show_help, "Show help");
                ui.add_space(8.0);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.checkbox(
                        &mut self.drawing.drag_dimensions_enabled,
                        "Allow dragging dimensions",
                    );
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.checkbox(
                        &mut self.drawing.drag_features_enabled,
                        "Allow dragging features",
                    );
                    ui.add_space(10.0);

                    let amt = ctx.animate_bool_with_time(
                        "error_display".into(),
                        self.drawing.last_solve_error.is_some(),
                        0.4,
                    );
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::RED.linear_multiply(amt));

                    if ui
                        .add(
                            egui::Label::new(format!(
                                "⚠ Solver inconsistency! avg: {:.3}mm",
                                self.drawing.last_solve_error.unwrap_or(0.0)
                            ))
                            .sense(egui::Sense::click()),
                        )
                        .clicked()
                    {
                        self.drawing.changed_in_ui();
                    };
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut main_widget =
                drawing::Widget::new(&mut self.drawing, &mut self.handler, &mut self.tools);
            if center {
                main_widget.center();
            }
            if zoom {
                main_widget.autozoom();
            }
            main_widget.show(ui);
        });

        detailer::Widget::new(
            &mut self.detailer_state,
            &mut self.drawing,
            &mut self.tools,
            &mut self.handler,
            &mut self.toasts,
        )
        .show(ctx, |type_name, ext, data| {
            pending_export = Some((type_name, ext, data));
        });

        helper::Widget::new(
            &mut self.helper_state,
            &mut self.show_help,
            &mut self.toasts,
        )
        .show(ctx);

        // egui::Window::new("📝 Memory")
        //     .resizable(false)
        //     .show(ctx, |ui| {
        //         ctx.memory_ui(ui);
        //         ctx.inspection_ui(ui);
        //     });

        self.toasts.show(ctx);

        if let Some((type_name, ext, data)) = pending_export {
            self.export_str_as(type_name, ext, data);
        }
    }
}
