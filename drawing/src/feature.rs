use super::{PaintParams, Viewport};

const POINT_SIZE: egui::Vec2 = egui::Vec2 { x: 4.5, y: 4.5 };

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum Feature {
    Point(f32, f32),
}

impl Default for Feature {
    fn default() -> Self {
        Feature::Point(0., 0.)
    }
}

impl super::DrawingFeature for Feature {
    fn bb(&self) -> egui::Rect {
        match self {
            Feature::Point(x, y) => egui::Rect {
                min: egui::Pos2 { x: *x, y: *y },
                max: egui::Pos2 { x: *x, y: *y },
            },
        }
    }

    fn screen_dist(&self, hp: egui::Pos2, vp: &Viewport) -> f32 {
        use crate::Feature::Point;

        match self {
            Point(x, y) => vp
                .translate_point(egui::Pos2 { x: *x, y: *y })
                .distance_sq(hp),
        }
    }

    fn paint(&self, _k: slotmap::DefaultKey, params: &PaintParams, painter: &egui::Painter) {
        match self {
            Feature::Point(_, _) => {
                painter.rect_filled(
                    params.vp.translate_rect(self.bb()).expand2(POINT_SIZE),
                    egui::Rounding::ZERO,
                    if params.selected {
                        params.colors.selected
                    } else {
                        if params.hovered {
                            params.colors.hover
                        } else {
                            params.colors.point
                        }
                    },
                );
            }
        }
    }
}
