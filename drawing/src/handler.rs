use super::{Data, Feature, FeatureKey, FeatureMeta};
use crate::tools::Toolbar;
use crate::{Axis, Constraint, ConstraintKey, ConstraintMeta, DimensionDisplay};

#[derive(Debug)]
pub enum ToolResponse {
    Handled,
    SwitchToPointer,
    NewPoint(egui::Pos2),
    NewLineSegment(FeatureKey, FeatureKey),
    NewArc(FeatureKey, FeatureKey),
    Delete(FeatureKey),

    NewFixedConstraint(FeatureKey),
    NewLineLengthConstraint(FeatureKey),
    NewLineCardinalConstraint(FeatureKey, bool), // true = horizontal
    NewPointLerp(FeatureKey, FeatureKey),        // point, line
    ConstraintDelete(ConstraintKey),
    NewEqual(FeatureKey, FeatureKey),
    NewParallelLine(FeatureKey, FeatureKey),

    DeleteGroup(usize),
}

#[derive(Debug, Default)]
pub struct Handler {}

impl Handler {
    pub fn handle(&mut self, drawing: &mut Data, tools: &mut Toolbar, c: ToolResponse) {
        match c {
            ToolResponse::Handled => {}
            ToolResponse::SwitchToPointer => {
                tools.clear();
            }
            ToolResponse::DeleteGroup(idx) => {
                drawing.groups.remove(idx);
            }
            ToolResponse::NewPoint(pos) => {
                let pos = drawing.vp.screen_to_point(pos);
                let p = Feature::Point(FeatureMeta::default(), pos.x, pos.y);

                if drawing.feature_exists(&p) {
                    return;
                }

                drawing.features.insert(p);
            }

            ToolResponse::NewLineSegment(p1, p2) => {
                let l = Feature::LineSegment(FeatureMeta::default(), p2, p1);

                if drawing.feature_exists(&l) {
                    return;
                }

                drawing.features.insert(l);
            }

            ToolResponse::NewArc(fk1, fk2) => {
                let (f1, f2) = (
                    drawing.features.get(fk1).unwrap(),
                    drawing.features.get(fk2).unwrap(),
                );
                let (p1, p2) = match (f1, f2) {
                    (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                        (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                    }
                    _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                };
                let mid = p1.lerp(p2, 0.5);

                let mid_fk =
                    drawing
                        .features
                        .insert(Feature::Point(FeatureMeta::default(), mid.x, mid.y));
                let a = Feature::Arc(FeatureMeta::default(), fk1, mid_fk, fk2);
                drawing.features.insert(a);

                tools.clear();
            }

            ToolResponse::Delete(k) => {
                drawing.delete_feature(k);
            }
            ToolResponse::ConstraintDelete(k) => {
                drawing.delete_constraint(k);
            }

            ToolResponse::NewFixedConstraint(k) => match drawing.features.get(k) {
                Some(Feature::Point(..)) => {
                    drawing.add_constraint(Constraint::Fixed(ConstraintMeta::default(), k, 0., 0.));

                    tools.clear();
                }
                _ => {}
            },
            ToolResponse::NewLineLengthConstraint(k) => match drawing.features.get(k) {
                Some(Feature::LineSegment(_, f1, f2)) => {
                    let (f1, f2) = (
                        drawing.features.get(*f1).unwrap(),
                        drawing.features.get(*f2).unwrap(),
                    );
                    let (p1, p2) = match (f1, f2) {
                        (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                            (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                        }
                        _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                    };

                    let d = p1.distance(p2);

                    drawing.add_constraint(Constraint::LineLength(
                        ConstraintMeta::default(),
                        k,
                        d,
                        None,
                        DimensionDisplay { x: 0., y: 35.0 },
                    ));

                    tools.clear();
                }
                _ => {}
            },
            ToolResponse::NewLineCardinalConstraint(k, is_horizontal) => {
                match drawing.features.get(k) {
                    Some(Feature::LineSegment(_, _f1, _f2)) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::LineAlongCardinal(
                            ConstraintMeta::default(),
                            k,
                            if is_horizontal {
                                Axis::LeftRight
                            } else {
                                Axis::TopBottom
                            },
                        ));

                        tools.clear();
                    }
                    _ => {}
                }
            }
            ToolResponse::NewPointLerp(p_fk, l_fk) => {
                match (drawing.features.get(p_fk), drawing.features.get(l_fk)) {
                    (Some(Feature::Point(..)), Some(Feature::LineSegment(..))) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::PointLerpLine(
                            ConstraintMeta::default(),
                            l_fk,
                            p_fk,
                            0.5,
                        ));

                        tools.clear();
                    }
                    _ => {}
                }
            }
            ToolResponse::NewEqual(l1, l2) => {
                match (drawing.features.get(l1), drawing.features.get(l2)) {
                    (Some(Feature::LineSegment(..)), Some(Feature::LineSegment(..))) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::LineLengthsEqual(
                            ConstraintMeta::default(),
                            l1,
                            l2,
                        ));

                        tools.clear();
                    }
                    _ => {}
                }
            }
            ToolResponse::NewParallelLine(l1, l2) => {
                match (drawing.features.get(l1), drawing.features.get(l2)) {
                    (Some(Feature::LineSegment(..)), Some(Feature::LineSegment(..))) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::LinesParallel(
                            ConstraintMeta::default(),
                            l1,
                            l2,
                        ));

                        tools.clear();
                    }
                    _ => {}
                }
            }
        }
    }
}
