use super::{Data, PaintParams, Viewport};
use slotmap::DefaultKey as K;

const POINT_SIZE: egui::Vec2 = egui::Vec2 { x: 4.5, y: 4.5 };

#[derive(Debug)]
pub struct LineSegment {
    pub p1: egui::Pos2,
    pub p2: egui::Pos2,
}

impl LineSegment {
    pub fn distance_to_point(&self, point: &egui::Pos2) -> f32 {
        let l2 = self.p1.distance_sq(self.p2);
        if l2 > -f32::EPSILON && l2 < f32::EPSILON {
            // If the line segment is just a point, return the distance between the point and that single point
            return self.p1.distance_sq(*point);
        }

        // Calculate the projection of the point onto the line segment
        let t = ((point.x - self.p1.x) * (self.p2.x - self.p1.x)
            + (point.y - self.p1.y) * (self.p2.y - self.p1.y))
            / l2;

        if t < 0.0 {
            // Closest point is p1
            self.p1.distance_sq(*point)
        } else if t > 1.0 {
            // Closest point is p2
            self.p2.distance_sq(*point)
        } else {
            // Closest point is between p1 and p2
            let projection = egui::Pos2 {
                x: self.p1.x + t * (self.p2.x - self.p1.x),
                y: self.p1.y + t * (self.p2.y - self.p1.y),
            };
            point.distance_sq(projection)
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum Feature {
    Point(f32, f32),
    LineSegment(K, K),
}

impl Default for Feature {
    fn default() -> Self {
        Feature::Point(0., 0.)
    }
}

impl Feature {
    pub fn is_point(&self) -> bool {
        matches!(self, Feature::Point(_, _))
    }

    pub fn depends_on(&self) -> [Option<K>; 2] {
        match self {
            Feature::Point(_, _) => [None, None],
            Feature::LineSegment(p1, p2) => [Some(*p1), Some(*p2)],
        }
    }

    pub fn bb(&self, drawing: &Data) -> egui::Rect {
        match self {
            Feature::Point(x, y) => egui::Rect {
                min: egui::Pos2 { x: *x, y: *y },
                max: egui::Pos2 { x: *x, y: *y },
            },
            Feature::LineSegment(p1, p2) => {
                let (p1, p2) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                );

                p1.bb(drawing).union(p2.bb(drawing))
            }
        }
    }

    pub fn screen_dist(&self, drawing: &Data, hp: egui::Pos2, vp: &Viewport) -> f32 {
        match self {
            Feature::Point(x, y) => vp
                .translate_point(egui::Pos2 { x: *x, y: *y })
                .distance_sq(hp),

            Feature::LineSegment(p1, p2) => {
                let (f1, f2) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                );
                let (p1, p2) = match (f1, f2) {
                    (Feature::Point(x1, y1), Feature::Point(x2, y2)) => (
                        vp.translate_point(egui::Pos2 { x: *x1, y: *y1 }),
                        vp.translate_point(egui::Pos2 { x: *x2, y: *y2 }),
                    ),
                    _ => panic!("unexpected subkey types: {:?} & {:?}", p1, p2),
                };

                LineSegment { p1, p2 }.distance_to_point(&hp)
            }
        }
    }

    pub fn paint(
        &self,
        drawing: &Data,
        _k: slotmap::DefaultKey,
        params: &PaintParams,
        painter: &egui::Painter,
    ) {
        match self {
            Feature::Point(_, _) => {
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

            Feature::LineSegment(p1, p2) => {
                let (f1, f2) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                );
                let (p1, p2) = match (f1, f2) {
                    (Feature::Point(x1, y1), Feature::Point(x2, y2)) => (
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
