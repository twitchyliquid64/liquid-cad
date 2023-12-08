#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Viewport {
    pub fn screen_to_point(&self, p: egui::Pos2) -> egui::Pos2 {
        egui::Pos2 {
            x: self.zoom * p.x + self.x,
            y: self.zoom * p.y + self.y,
        }
    }
    pub fn translate_point(&self, p: egui::Pos2) -> egui::Pos2 {
        egui::Pos2 {
            x: (p.x - self.x) / self.zoom,
            y: (p.y - self.y) / self.zoom,
        }
    }
    pub fn translate_rect(&self, r: egui::Rect) -> egui::Rect {
        egui::Rect {
            min: self.translate_point(r.min),
            max: self.translate_point(r.max),
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            zoom: 1.,
        }
    }
}
