use crate::{Feature, FeatureKey};

slotmap::new_key_type! {
    pub struct ConstraintKey;
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct ConstraintMeta {}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum Constraint {
    Fixed(ConstraintMeta, FeatureKey, f32, f32),
}

impl Constraint {
    pub fn affecting_features(&self) -> Vec<FeatureKey> {
        use Constraint::Fixed;
        match self {
            Fixed(_, fk, _, _) => vec![fk.clone()],
        }
    }

    pub fn valid_for_feature(&self, ft: &Feature) -> bool {
        use Constraint::Fixed;
        match self {
            Fixed(..) => matches!(ft, &Feature::Point(..)),
        }
    }

    pub fn conflicts(&self, other: &Constraint) -> bool {
        use Constraint::Fixed;
        match (self, other) {
            (Fixed(_, f1, _, _), Fixed(_, f2, _, _)) => f1 == f2,
            _ => false,
        }
    }

    pub fn paint(
        &self,
        drawing: &crate::Data,
        _k: ConstraintKey,
        params: &crate::PaintParams,
        painter: &egui::Painter,
    ) {
        use Constraint::Fixed;
        match self {
            Fixed(_, k, _, _) => {
                if let Some(Feature::Point(_, x, y)) = drawing.features.get(*k) {
                    let layout = painter.layout_no_wrap(
                        "( )".to_string(),
                        egui::FontId::monospace(12.),
                        params.colors.text,
                    );

                    let c = params.vp.translate_point(egui::Pos2 { x: *x, y: *y });
                    painter.circle_stroke(
                        c,
                        7.,
                        egui::Stroke {
                            width: 1.,
                            color: params.colors.text,
                        },
                    );
                };
            }
        }
    }
}
