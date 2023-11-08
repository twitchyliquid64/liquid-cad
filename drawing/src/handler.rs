use super::{Data, Feature, FeatureKey, FeatureMeta};
use crate::tools::Toolbar;
use crate::{Constraint, ConstraintKey, ConstraintMeta};

#[derive(Debug)]
pub enum ToolResponse {
    Handled,
    SwitchToPointer,
    NewPoint(egui::Pos2),
    NewLineSegment(egui::Pos2, egui::Pos2),
    Delete(FeatureKey),

    NewFixedConstraint(FeatureKey),
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
        }
    }
}
