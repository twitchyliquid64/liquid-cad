use super::{Data, Feature, FeatureMeta};
use crate::tools::ToolResponse;

#[derive(Debug, Default)]
pub struct Handler {}

impl Handler {
    pub fn handle(&mut self, drawing: &mut Data, c: ToolResponse) {
        match c {
            ToolResponse::Handled => {}
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
        }
    }
}
