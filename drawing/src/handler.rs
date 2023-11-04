use super::{Data, Feature};
use crate::tools::ToolResponse;

#[derive(Debug, Default)]
pub struct Handler {}

impl super::CommandHandler<Feature, ToolResponse> for Handler {
    fn handle(&mut self, drawing: &mut Data<Feature>, c: ToolResponse) {
        match c {
            ToolResponse::Handled => {}
            ToolResponse::NewPoint(pos) => {
                let pos = drawing.vp.screen_to_point(pos);
                drawing.features.insert(Feature::Point(pos.x, pos.y));
            }

            ToolResponse::NewLineSegment(p1, p2) => {
                // points correspond to the exact coordinates of an existing point
                let (f1, f2) = (
                    drawing.find_point_at(p1).unwrap(),
                    drawing.find_point_at(p2).unwrap(),
                );

                drawing.features.insert(Feature::LineSegment(f1, f2));
            }
        }
    }
}
