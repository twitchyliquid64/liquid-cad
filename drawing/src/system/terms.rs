use crate::{ConstraintKey, FeatureKey};
use std::collections::HashMap;

/// Specialization of a term.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum TermType {
    #[default]
    ScalarDistance,
    PositionX,
    PositionY,
}

/// Represents a term in the system of equations.
#[derive(Eq, Hash, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct TermRef {
    base: usize,
    pub(crate) t: TermType,
    pub(crate) for_feature: Option<FeatureKey>,
}

impl PartialEq for TermRef {
    fn eq(&self, other: &Self) -> bool {
        return self.t == other.t && self.base == other.base;
    }
}

impl std::fmt::Display for TermRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TermType::{PositionX, PositionY, ScalarDistance};
        match self.t {
            ScalarDistance => write!(f, "d{}", self.base),
            PositionX => write!(f, "x{}", self.base),
            PositionY => write!(f, "y{}", self.base),
        }
    }
}

impl Into<eq::Variable> for &TermRef {
    fn into(self) -> eq::Variable {
        format!("{}", self).as_str().into()
    }
}

/// Allocates terms for parameters of different entities which
/// need to be referenced or solved.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct TermAllocator {
    top: usize,
    by_feature: HashMap<FeatureKey, usize>,
    free: Vec<usize>,
}

impl TermAllocator {
    pub fn get_feature_term(&mut self, fk: FeatureKey, t: TermType) -> TermRef {
        if let Some(base) = self.by_feature.get(&fk) {
            return TermRef {
                t,
                base: *base,
                for_feature: Some(fk),
            };
        }

        let base = self.alloc_base();
        self.by_feature.insert(fk, base);
        TermRef {
            t,
            base,
            for_feature: Some(fk),
        }
    }

    fn alloc_base(&mut self) -> usize {
        if let Some(base) = self.free.pop() {
            return base;
        }

        let out = self.top;
        self.top += 1;
        out
    }

    /// Records deletion of a feature, so its index can be used.
    pub fn delete_feature(&mut self, fk: FeatureKey) {
        if let Some(base) = self.by_feature.remove(&fk) {
            self.free.push(base);
        }
    }

    /// Records deletion of a constraint, so its index can be used.
    pub fn delete_constraint(&mut self, fk: ConstraintKey) {}
}
