use super::{Data, PaintParams, Viewport};
use crate::l::LineSegment;

slotmap::new_key_type! {
    pub struct FeatureKey;
}

const POINT_SIZE: egui::Vec2 = egui::Vec2 { x: 4.5, y: 4.5 };

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct FeatureMeta {}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum Feature {
    Point(FeatureMeta, f32, f32),
    LineSegment(FeatureMeta, FeatureKey, FeatureKey),
}

impl Default for Feature {
    fn default() -> Self {
        Feature::Point(FeatureMeta::default(), 0., 0.)
    }
}

impl PartialEq<Feature> for Feature {
    fn eq(&self, other: &Feature) -> bool {
        use Feature::{LineSegment, Point};
        match (self, other) {
            (Point(_, x1, y1), Point(_, x2, y2)) => x1 == x2 && y1 == y2,
            (LineSegment(_, p00, p01), LineSegment(_, p10, p11)) => {
                (p00 == p10 && p01 == p11) || (p00 == p11 && p01 == p10)
            }
            _ => false,
        }
    }
}

impl Feature {
    pub fn is_point(&self) -> bool {
        matches!(self, Feature::Point(_, _, _))
    }

    pub fn depends_on(&self) -> [Option<FeatureKey>; 2] {
        match self {
            Feature::Point(_, _, _) => [None, None],
            Feature::LineSegment(_, p1, p2) => [Some(*p1), Some(*p2)],
        }
    }

    pub fn bb(&self, drawing: &Data) -> egui::Rect {
        match self {
            Feature::Point(_, x, y) => egui::Rect {
                min: egui::Pos2 { x: *x, y: *y },
                max: egui::Pos2 { x: *x, y: *y },
            },
            Feature::LineSegment(_, p1, p2) => {
                let (p1, p2) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                );

                p1.bb(drawing).union(p2.bb(drawing))
            }
        }
    }

    pub fn screen_dist_sq(&self, drawing: &Data, hp: egui::Pos2, vp: &Viewport) -> f32 {
        match self {
            Feature::Point(_, x, y) => vp
                .translate_point(egui::Pos2 { x: *x, y: *y })
                .distance_sq(hp),

            Feature::LineSegment(_, p1, p2) => {
                let (f1, f2) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                );
                let (p1, p2) = match (f1, f2) {
                    (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => (
                        vp.translate_point(egui::Pos2 { x: *x1, y: *y1 }),
                        vp.translate_point(egui::Pos2 { x: *x2, y: *y2 }),
                    ),
                    _ => panic!("unexpected subkey types: {:?} & {:?}", p1, p2),
                };

                LineSegment { p1, p2 }.distance_to_point_sq(&hp)
            }
        }
    }

    pub fn paint(
        &self,
        drawing: &Data,
        _k: FeatureKey,
        params: &PaintParams,
        painter: &egui::Painter,
    ) {
        match self {
            Feature::Point(_, _, _) => {
                painter.rect_filled(
                    params
                        .vp
                        .translate_rect(self.bb(drawing))
                        .expand2(POINT_SIZE),
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

            Feature::LineSegment(_, p1, p2) => {
                let (f1, f2) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                );
                let (p1, p2) = match (f1, f2) {
                    (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => (
                        params.vp.translate_point(egui::Pos2 { x: *x1, y: *y1 }),
                        params.vp.translate_point(egui::Pos2 { x: *x2, y: *y2 }),
                    ),
                    _ => panic!("unexpected subkey types: {:?} & {:?}", p1, p2),
                };

                painter.line_segment(
                    [p1, p2],
                    egui::Stroke {
                        width: 1.,
                        color: if params.selected {
                            params.colors.selected
                        } else {
                            if params.hovered {
                                params.colors.hover
                            } else {
                                params.colors.line
                            }
                        },
                    },
                )
            }
        }
    }
}
