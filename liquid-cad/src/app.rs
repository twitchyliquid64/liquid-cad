use detailer;
use drawing;
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
    toasts: egui_toast::Toasts,

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
        let toasts = egui_toast::Toasts::new()
            .anchor(egui::Align2::RIGHT_BOTTOM, (-10.0, -10.0)) // 10 units from the bottom right corner
            .direction(egui::Direction::BottomUp);

        let last_path = None;
        let wasm_open_channel = channel();

        Self {
            drawing,
            handler,
            tools,
            detailer_state,
            toasts,
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
                }
            } else {
                println!("nothing read from storage");
            }
        }

        app
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
                    let _ = file.write(
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
                    });
                }
                ui.add_space(8.0);

                ui.menu_button("Drawing", |ui| {
                    if ui.button("Clear selection   (Esc)").clicked() {
                        self.drawing.selection_clear();
                    }
                    if ui.button("Select all   (Ctrl-A)").clicked() {
                        self.drawing.select_all();
                    }
                    if ui.button("Solve step").clicked() {
                        self.drawing.changed_in_ui();
                    }
                });
                ui.add_space(8.0);
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
            &mut self.toasts,
        )
        .show(ctx);

        self.toasts.show(ctx);
    }
}
