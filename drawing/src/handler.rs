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
        }
    }
}
