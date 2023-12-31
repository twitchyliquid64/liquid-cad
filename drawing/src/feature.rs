use super::{Data, PaintParams, Viewport};
use crate::l::{Arc, LineSegment};
use std::collections::HashMap;

slotmap::new_key_type! {
    pub struct FeatureKey;
}

const POINT_SIZE: egui::Vec2 = egui::Vec2 { x: 4.5, y: 4.5 };

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct FeatureMeta {
    pub construction: bool,
}

impl FeatureMeta {
    pub fn default_construction() -> Self {
        Self { construction: true }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SerializedFeature {
    pub kind: String,
    pub meta: FeatureMeta,
    pub using_idx: Vec<usize>,

    pub x: f32,
    pub y: f32,
    pub r: f32,
}

#[derive(Debug, Clone)]
pub enum Feature {
    Point(FeatureMeta, f32, f32),
    LineSegment(FeatureMeta, FeatureKey, FeatureKey),
    Arc(FeatureMeta, FeatureKey, FeatureKey, FeatureKey), // start, center, end
    Circle(FeatureMeta, FeatureKey, f32),                 // center, radius
}

impl Default for Feature {
    fn default() -> Self {
        Feature::Point(FeatureMeta::default(), 0., 0.)
    }
}

impl PartialEq<Feature> for Feature {
    fn eq(&self, other: &Feature) -> bool {
        use Feature::{Arc, Circle, LineSegment, Point};
        match (self, other) {
            (Point(_, x1, y1), Point(_, x2, y2)) => x1 == x2 && y1 == y2,
            (LineSegment(_, p00, p01), LineSegment(_, p10, p11)) => {
                (p00 == p10 && p01 == p11) || (p00 == p11 && p01 == p10)
            }
            (Arc(_, p00, p01, p02), Arc(_, p10, p11, p12)) => {
                p01 == p11 && ((p00 == p10 && p02 == p12) || (p00 == p12 && p02 == p10))
            }
            (Circle(_, p0, r0, ..), Circle(_, p1, r1, ..)) => p0 == p1 && (r1 - r0).abs() < 0.005,
            _ => false,
        }
    }
}

impl Feature {
    pub fn is_point(&self) -> bool {
        matches!(self, Feature::Point(_, _, _))
    }
    pub fn is_construction(&self) -> bool {
        match self {
            Feature::Point(meta, ..) => meta.construction,
            Feature::LineSegment(meta, ..) => meta.construction,
            Feature::Arc(meta, ..) => meta.construction,
            Feature::Circle(meta, ..) => meta.construction,
        }
    }

    pub fn depends_on(&self) -> [Option<FeatureKey>; 3] {
        match self {
            Feature::Point(_, _, _) => [None, None, None],
            Feature::LineSegment(_, p1, p2) => [Some(*p1), Some(*p2), None],
            Feature::Arc(_, p1, p2, p3) => [Some(*p1), Some(*p2), Some(*p3)],
            Feature::Circle(_, p, ..) => [Some(*p), None, None],
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
            Feature::Arc(_, p1, p2, p3) => {
                // TODO: super incorrect, fix this
                let (p1, p2, p3) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                    drawing.features.get(*p3).unwrap(),
                );

                p1.bb(drawing).union(p2.bb(drawing).union(p3.bb(drawing)))
            }
            Feature::Circle(_, p, r, ..) => {
                let p = drawing.features.get(*p).unwrap();
                p.bb(drawing).expand(*r)
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

            Feature::Arc(_, p1, p2, p3) => {
                let (f1, f2, f3) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                    drawing.features.get(*p3).unwrap(),
                );
                let (start, center, end) = match (f1, f2, f3) {
                    (
                        Feature::Point(_, x1, y1),
                        Feature::Point(_, x2, y2),
                        Feature::Point(_, x3, y3),
                    ) => (
                        vp.translate_point(egui::Pos2 { x: *x1, y: *y1 }),
                        vp.translate_point(egui::Pos2 { x: *x2, y: *y2 }),
                        vp.translate_point(egui::Pos2 { x: *x3, y: *y3 }),
                    ),
                    _ => panic!("unexpected subkey types: {:?} & {:?} & {:?}", p1, p2, p3),
                };

                Arc { start, center, end }.distance_to_point_sq(&hp)
            }

            Feature::Circle(_, p, r, ..) => {
                let p = vp.translate_point(match drawing.features.get(*p).unwrap() {
                    Feature::Point(_, x1, y1) => egui::Pos2 { x: *x1, y: *y1 },
                    _ => unreachable!(),
                });
                let (x_diff, y_diff) = (hp.x - p.x, hp.y - p.y);

                ((x_diff.powi(2) + y_diff.powi(2)).sqrt() - r / vp.zoom).powi(2)
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
            Feature::Point(meta, _, _) => {
                painter.rect_filled(
                    params
                        .vp
                        .translate_rect(self.bb(drawing))
                        .expand2(POINT_SIZE),
                    egui::Rounding::ZERO,
                    if params.selected {
                        params.colors.selected
                    } else if params.hovered {
                        params.colors.hover
                    } else if meta.construction {
                        params.colors.point.gamma_multiply(0.35)
                    } else {
                        params.colors.point
                    },
                );
            }

            Feature::LineSegment(meta, p1, p2) => {
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
                        } else if params.hovered {
                            params.colors.hover
                        } else if meta.construction {
                            params.colors.line.gamma_multiply(0.35)
                        } else {
                            params.colors.line
                        },
                    },
                )
            }

            Feature::Arc(meta, p1, ..) => {
                let color = if params.selected {
                    params.colors.selected
                } else if params.hovered {
                    params.colors.hover
                } else if meta.construction {
                    params.colors.line.gamma_multiply(0.35)
                } else {
                    params.colors.line
                };
                let stroke = egui::Stroke::new(1.0, color);

                if let Some(a) = self.kurbo_arc(drawing) {
                    let start = drawing.features.get(*p1).unwrap().start_point(drawing);

                    let mut last = (start.x, start.y);
                    a.to_cubic_beziers(0.1, |p1, p2, p| {
                        let shape = egui::epaint::CubicBezierShape::from_points_stroke(
                            [
                                params.vp.translate_point(last.into()),
                                params.vp.translate_point((p1.x as f32, p1.y as f32).into()),
                                params.vp.translate_point((p2.x as f32, p2.y as f32).into()),
                                params.vp.translate_point((p.x as f32, p.y as f32).into()),
                            ],
                            false,
                            egui::Color32::TRANSPARENT,
                            stroke,
                        );
                        painter.add(shape);
                        last = (p.x as f32, p.y as f32);
                    })
                }
            }

            Feature::Circle(meta, p, r, ..) => {
                let f = drawing.features.get(*p).unwrap();
                let p = match f {
                    Feature::Point(_, x1, y1) => {
                        params.vp.translate_point(egui::Pos2 { x: *x1, y: *y1 })
                    }
                    _ => panic!("unexpected subkey type: {:?}", f),
                };

                painter.circle_stroke(
                    p,
                    *r / params.vp.zoom,
                    egui::Stroke {
                        width: 1.,
                        color: if params.selected {
                            params.colors.selected
                        } else if params.hovered {
                            params.colors.hover
                        } else if meta.construction {
                            params.colors.line.gamma_multiply(0.35)
                        } else {
                            params.colors.line
                        },
                    },
                )
            }
        }
    }

    /// Serialize returns a structure suitable for serialization to disk. Any point
    /// which maybe referenced from the current feature must be present in fk_to_idx.
    pub fn serialize(
        &self,
        fk_to_idx: &HashMap<FeatureKey, usize>,
    ) -> Result<SerializedFeature, ()> {
        match self {
            Feature::Point(meta, x, y) => Ok(SerializedFeature {
                kind: "pt".to_string(),
                meta: meta.clone(),
                using_idx: vec![],
                x: *x,
                y: *y,
                ..SerializedFeature::default()
            }),

            Feature::LineSegment(meta, p1, p2) => {
                let (p1_idx, p2_idx) = (fk_to_idx.get(p1).ok_or(())?, fk_to_idx.get(p2).ok_or(())?);

                Ok(SerializedFeature {
                    kind: "line".to_string(),
                    meta: meta.clone(),
                    using_idx: vec![*p1_idx, *p2_idx],
                    ..SerializedFeature::default()
                })
            }

            Feature::Arc(meta, start, center, end) => {
                let (start_idx, center_idx, end_idx) = (
                    fk_to_idx.get(start).ok_or(())?,
                    fk_to_idx.get(center).ok_or(())?,
                    fk_to_idx.get(end).ok_or(())?,
                );

                Ok(SerializedFeature {
                    kind: "arc".to_string(),
                    meta: meta.clone(),
                    using_idx: vec![*start_idx, *center_idx, *end_idx],
                    ..SerializedFeature::default()
                })
            }

            Feature::Circle(meta, p, r) => {
                let p_idx = fk_to_idx.get(p).ok_or(())?;

                Ok(SerializedFeature {
                    kind: "circle".to_string(),
                    meta: meta.clone(),
                    using_idx: vec![*p_idx],
                    r: *r,
                    ..SerializedFeature::default()
                })
            }
        }
    }

    pub fn deserialize(
        sf: SerializedFeature,
        idx_to_fk: &HashMap<usize, FeatureKey>,
    ) -> Result<Self, ()> {
        match sf.kind.as_str() {
            "pt" => Ok(Self::Point(sf.meta, sf.x, sf.y)),
            "line" => {
                if sf.using_idx.len() < 2 {
                    return Err(());
                }
                Ok(Self::LineSegment(
                    sf.meta,
                    *idx_to_fk.get(&sf.using_idx[0]).ok_or(())?,
                    *idx_to_fk.get(&sf.using_idx[1]).ok_or(())?,
                ))
            }
            "arc" => {
                if sf.using_idx.len() < 3 {
                    return Err(());
                }
                Ok(Self::Arc(
                    sf.meta,
                    *idx_to_fk.get(&sf.using_idx[0]).ok_or(())?,
                    *idx_to_fk.get(&sf.using_idx[1]).ok_or(())?,
                    *idx_to_fk.get(&sf.using_idx[2]).ok_or(())?,
                ))
            }
            "circle" => {
                if sf.using_idx.len() < 1 {
                    return Err(());
                }
                Ok(Self::Circle(
                    sf.meta,
                    *idx_to_fk.get(&sf.using_idx[0]).ok_or(())?,
                    sf.r,
                ))
            }
            _ => Err(()),
        }
    }

    fn kurbo_arc(&self, drawing: &Data) -> Option<kurbo::Arc> {
        match self {
            Feature::Arc(_, p1, p2, p3, ..) => {
                let (f1, f2, f3) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                    drawing.features.get(*p3).unwrap(),
                );
                let (start, center, end) = match (f1, f2, f3) {
                    (
                        Feature::Point(_, x1, y1),
                        Feature::Point(_, x2, y2),
                        Feature::Point(_, x3, y3),
                    ) => (
                        egui::Pos2 { x: *x1, y: *y1 },
                        egui::Pos2 { x: *x2, y: *y2 },
                        egui::Pos2 { x: *x3, y: *y3 },
                    ),
                    _ => panic!("unexpected subkey types: {:?} & {:?} & {:?}", p1, p2, p3),
                };
                let r = (start.distance(center) as f64, end.distance(center) as f64);

                kurbo::Arc::from_svg_arc(&kurbo::SvgArc {
                    from: (start.x as f64, start.y as f64).into(),
                    to: (end.x as f64, end.y as f64).into(),
                    radii: r.into(),
                    sweep: true,
                    x_rotation: 0.0,
                    large_arc: {
                        let (d_start, d_end) = (start - center, end - center);
                        let dcross = d_start.x * d_end.y - d_end.x * d_start.y;
                        dcross < 0.0
                    },
                })
            }
            _ => None,
        }
    }

    pub fn bezier_path(&self, drawing: &Data) -> kurbo::BezPath {
        let mut out = kurbo::BezPath::default();

        use kurbo::Shape;
        match self {
            Feature::Point(_, x, y, ..) => {
                out.move_to(kurbo::Point {
                    x: *x as f64,
                    y: *y as f64,
                });
            }
            Feature::LineSegment(_, p1, p2, ..) => {
                let (f1, f2) = (
                    drawing.features.get(*p1).unwrap(),
                    drawing.features.get(*p2).unwrap(),
                );
                let (p1, p2) = match (f1, f2) {
                    (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                        (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                    }
                    _ => panic!("unexpected subkey types: {:?} & {:?}", p1, p2),
                };
                out.move_to(kurbo::Point {
                    x: p1.x as f64,
                    y: p1.y as f64,
                });
                out.line_to(kurbo::Point {
                    x: p2.x as f64,
                    y: p2.y as f64,
                });
            }
            Feature::Arc(..) => {
                if let Some(a) = self.kurbo_arc(drawing) {
                    out = a.into_path(0.1);
                }
            }
            Feature::Circle(_, p_center, radius, ..) => {
                let p = drawing
                    .features
                    .get(*p_center)
                    .unwrap()
                    .start_point(drawing);

                out = kurbo::Circle::new(
                    kurbo::Point {
                        x: p.x as f64,
                        y: p.y as f64,
                    },
                    *radius as f64,
                )
                .into_path(0.1);
            }
        };
        out
    }

    pub fn start_point(&self, drawing: &Data) -> egui::Pos2 {
        match self {
            Feature::Point(_, x, y, ..) => egui::Pos2 { x: *x, y: *y },
            Feature::LineSegment(_, p1, ..) => {
                drawing.features.get(*p1).unwrap().start_point(drawing)
            }
            Feature::Arc(_, p_start, ..) => {
                drawing.features.get(*p_start).unwrap().start_point(drawing)
            }
            Feature::Circle(_, p_center, radius, ..) => {
                drawing
                    .features
                    .get(*p_center)
                    .unwrap()
                    .start_point(drawing)
                    + egui::Vec2 { x: *radius, y: 0.0 }
            }
        }
    }

    pub fn end_point(&self, drawing: &Data) -> egui::Pos2 {
        match self {
            Feature::Point(_, x, y, ..) => egui::Pos2 { x: *x, y: *y },
            Feature::LineSegment(_, _, p2, ..) => {
                drawing.features.get(*p2).unwrap().start_point(drawing)
            }
            Feature::Arc(_, _p_start, _p_middle, p_end, ..) => {
                drawing.features.get(*p_end).unwrap().start_point(drawing)
            }
            Feature::Circle(_, p_center, radius, ..) => {
                drawing
                    .features
                    .get(*p_center)
                    .unwrap()
                    .start_point(drawing)
                    + egui::Vec2 { x: *radius, y: 0.0 }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::Key;

    #[test]
    fn serialize() {
        assert_eq!(
            Feature::Point(FeatureMeta::default(), 1.5, 2.5).serialize(&HashMap::new()),
            Ok(SerializedFeature {
                kind: "pt".to_string(),
                meta: FeatureMeta::default(),
                using_idx: vec![],
                x: 1.5,
                y: 2.5,
                ..SerializedFeature::default()
            }),
        );

        let point_key = FeatureKey::null();

        assert_eq!(
            Feature::LineSegment(FeatureMeta::default(), point_key, point_key)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedFeature {
                kind: "line".to_string(),
                meta: FeatureMeta::default(),
                using_idx: vec![42, 42],
                ..SerializedFeature::default()
            }),
        );

        assert_eq!(
            Feature::Arc(FeatureMeta::default(), point_key, point_key, point_key)
                .serialize(&HashMap::from([(point_key, 22)])),
            Ok(SerializedFeature {
                kind: "arc".to_string(),
                meta: FeatureMeta::default(),
                using_idx: vec![22, 22, 22],
                ..SerializedFeature::default()
            }),
        );

        // Missing
        assert_eq!(
            Feature::LineSegment(FeatureMeta::default(), point_key, point_key)
                .serialize(&HashMap::new()),
            Err(()),
        );

        assert_eq!(
            Feature::Circle(FeatureMeta::default(), point_key, 6.9)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedFeature {
                kind: "circle".to_string(),
                meta: FeatureMeta::default(),
                using_idx: vec![42],
                r: 6.9,
                ..SerializedFeature::default()
            }),
        );
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            Feature::deserialize(
                SerializedFeature {
                    kind: "pt".to_string(),
                    x: 1.5,
                    y: 2.5,
                    ..SerializedFeature::default()
                },
                &HashMap::new()
            ),
            Ok(Feature::Point(FeatureMeta::default(), 1.5, 2.5)),
        );
        assert_eq!(
            Feature::deserialize(
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![3, 3],
                    ..SerializedFeature::default()
                },
                &HashMap::from([(3, FeatureKey::null())]),
            ),
            Ok(Feature::LineSegment(
                FeatureMeta::default(),
                FeatureKey::null(),
                FeatureKey::null()
            )),
        );
        assert_eq!(
            Feature::deserialize(
                SerializedFeature {
                    kind: "arc".to_string(),
                    using_idx: vec![1, 1, 1],
                    ..SerializedFeature::default()
                },
                &HashMap::from([(1, FeatureKey::null())]),
            ),
            Ok(Feature::Arc(
                FeatureMeta::default(),
                FeatureKey::null(),
                FeatureKey::null(),
                FeatureKey::null()
            )),
        );
        assert_eq!(
            Feature::deserialize(
                SerializedFeature {
                    kind: "circle".to_string(),
                    using_idx: vec![1],
                    r: 6.9,
                    ..SerializedFeature::default()
                },
                &HashMap::from([(1, FeatureKey::null())]),
            ),
            Ok(Feature::Circle(
                FeatureMeta::default(),
                FeatureKey::null(),
                6.9,
            )),
        );
    }
}
