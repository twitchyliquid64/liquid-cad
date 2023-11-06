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
}
