use egui::text::LayoutJob;

const HOTKEY_BACKGROUND_WHITENESS: u8 = 39;

#[derive(Debug, Default, Clone)]
pub struct State {
    uij: Option<LayoutJob>,
}

pub struct Widget<'a> {
    state: &'a mut State,
    open: &'a mut bool,
    toasts: &'a mut egui_toast::Toasts,
}

impl<'a> Widget<'a> {
    pub fn new(
        state: &'a mut State,
        open: &'a mut bool,
        toasts: &'a mut egui_toast::Toasts,
    ) -> Self {
        Widget {
            state,
            open,
            toasts,
        }
    }

    pub fn show(self, ctx: &egui::Context) {
        let window = egui::Window::new("Liquid-CAD Help")
            .id(egui::Id::new("help_window"))
            .resizable(true)
            .collapsible(true)
            .vscroll(true)
            .open(self.open)
            .min_width(350.)
            .default_height(1750.)
            .pivot(egui::Align2::RIGHT_BOTTOM);

        window.show(ctx, |ui| {
            ui.label("Welcome to Liquid-CAD!! Liquid-CAD is a 2d, constraint-solving CAD intended to help you rapidly prototype simple parts, such as mounting plates or laser-cut geometry.");

            ui.add_space(8.0);

            egui::CollapsingHeader::new(egui::RichText::new("Getting started").heading())
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(self.state.getting_started_layout_job(ui));
                });

            egui::CollapsingHeader::new(egui::RichText::new("Constraints reference").heading())
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("todo :)");
                });

            egui::CollapsingHeader::new(egui::RichText::new("Exporting").heading())
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("TL;DR: Add geometry into groups (detailer pane -> groups) corresponding to whether its on the outside of the part (boundary group) or cutouts of the part (interior groups - plural). Then press an export button there.");
                    ui.label("todo :)");
                });

            egui::CollapsingHeader::new(egui::RichText::new("Tips & Tricks").heading())
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("1. Use at least one fixed-point constraint (i.e. for your zero co-ordinate). It really helps solving.");
                    ui.label("2. Set 'construction' on geometry you've used as scaffolding (by checking the construction checkbox in the detailer -> selection view)");
                    ui.label("todo :)");
                });
        });
    }
}

impl State {
    fn getting_started_layout_job(&mut self, ui: &egui::Ui) -> LayoutJob {
        if let Some(uij) = &self.uij {
            return uij.clone();
        }
        let base = egui::TextFormat {
            font_id: egui::TextStyle::Body.resolve(ui.style()),
            ..Default::default()
        };

        let mut uij = LayoutJob::default();
        uij.append(
            "There are three main areas in the UI:\n\n - the ",
            0.0,
            base.clone(),
        );
        uij.append(
            "drawing area",
            0.0,
            egui::TextFormat {
                italics: true,
                ..base.clone()
            },
        );
        uij.append(
            ", which makes up the majority of the screen\n - the ",
            0.0,
            base.clone(),
        );
        uij.append(
            "toolbox",
            0.0,
            egui::TextFormat {
                italics: true,
                ..base.clone()
            },
        );
        uij.append(
            " (upper left), which contains the tools for manipulating your drawing, and\n - the ",
            0.0,
            base.clone(),
        );
        uij.append(
            "detailer pane",
            0.0,
            egui::TextFormat {
                italics: true,
                ..base.clone()
            },
        );
        uij.append(
            " (upper right), which lets you drill down into the details of selected elements as well as global settings.\n\n",
            0.0,
            base.clone(),
        );

        uij.append(
            "Start by using the points tool to create edges you will later join with lines. The points tool is the top-left-most tool in the toolbox, but you can equip it quickly with the hotkey ",
            0.0,
            base.clone(),
        );
        uij.append(
            " P ",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(
            ". (You can quickly exit tools and selections by pressing ",
            0.0,
            base.clone(),
        );
        uij.append(
            "ESC",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(").\n", 0.0, base.clone());

        uij.append(
            "Use your right-mouse button to pan about your drawing, and the scroll-wheel to zoom in and out.\n",
            0.0,
            base.clone(),
        );

        uij.append("Next, use the line tool (hotkey ", 0.0, base.clone());
        uij.append(
            " L ",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(
            ") to connect points with lines, by clicking on a starting point and then any number of following points. You can use the arc tool (hotkey ",
            0.0,
            base.clone(),
        );
        uij.append(
            " R ",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(") in the same fashion.\n", 0.0, base.clone());

        uij.append(
            "To inspect any geometry, select it by dragging with the left mouse (or clicking it). You can delete your current selection with the ",
            0.0,
            base.clone(),
        );
        uij.append(
            "DEL",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(" key.\n\n", 0.0, base.clone());

        uij.append(
            "At this stage, you should have a drawing composed of a bunch of haphazard lines or whatever. Lets go-ahead and make it something meaningful by constraining it to form a part we actually want!\n",
            0.0,
            base.clone(),
        );
        uij.append(
            "Lets start with some simple constraints: horizontal/vertical (hotkeys ",
            0.0,
            base.clone(),
        );
        uij.append(
            " H ",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(" / ", 0.0, base.clone());
        uij.append(
            " V ",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(
            "). Equip the horizontal/vertical tool and click a line of your choice. Boom! It should now be always horizontal or vertical, even if you drag the line or any connected points around! And if you select the line, you should see your new constraint show up in the selections tab of the detailer pane (along with a delete button if you change your mind).\n\n",
            0.0,
            base.clone(),
        );

        uij.append(
            "I'll let you explore on your own to discover the other constraints, but ill mention two other important ones: the dimension constraint (hotkey ",
            0.0,
            base.clone(),
        );
        uij.append(
            " D ",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(
            ") which lets you set the length of lines (and the radius of circles, etc), and the fixed-point constraint (hotkey ",
            0.0,
            base.clone(),
        );
        uij.append(
            " S ",
            0.0,
            egui::TextFormat {
                background: egui::Color32::from_gray(HOTKEY_BACKGROUND_WHITENESS),
                ..base.clone()
            },
        );
        uij.append(
            ") which positions a point at some exact set of co-ordinates. Its a really good idea to set a point to (0,0) early on, it really helps the constraint solver.",
            0.0,
            base.clone(),
        );

        self.uij = Some(uij.clone());
        uij
    }
}
