use super::{Data, Feature, FeatureKey, FeatureMeta};
use crate::tools::Toolbar;
use crate::{Axis, Constraint, ConstraintKey, ConstraintMeta, DimensionDisplay};

#[derive(Debug)]
pub enum ToolResponse {
    Handled,
    SwitchToPointer,
    NewPoint(egui::Pos2),
    NewLineSegment(egui::Pos2, egui::Pos2),
    Delete(FeatureKey),

    NewFixedConstraint(FeatureKey),
    NewLineLengthConstraint(FeatureKey),
    NewLineCardinalConstraint(FeatureKey, bool), // true = horizontal
    ConstraintDelete(ConstraintKey),
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
            ToolResponse::NewPoint(pos) => {
                let pos = drawing.vp.screen_to_point(pos);
                let p = Feature::Point(FeatureMeta::default(), pos.x, pos.y);

                // Make sure it doesnt already exist
                for v in drawing.features.values() {
                    if v == &p {
                        return;
                    }
                }

                drawing.features.insert(p);
            }

            ToolResponse::NewLineSegment(p1, p2) => {
                // points correspond to the exact coordinates of an existing point
                let (f1, f2) = (
                    drawing.find_point_at(p1).unwrap(),
                    drawing.find_point_at(p2).unwrap(),
                );
                let l = Feature::LineSegment(FeatureMeta::default(), f2, f1);

                // Make sure it doesnt already exist
                for v in drawing.features.values() {
                    if v == &l {
                        return;
                    }
                }

                drawing.features.insert(l);
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
        }
    }
}
