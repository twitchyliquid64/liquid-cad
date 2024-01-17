use crate::{ConstraintKey, FeatureKey};
use std::collections::HashMap;

/// Specialization of a term.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum TermType {
    #[default]
    ScalarDistance,
    PositionX,
    PositionY,
    ScalarRadius,
    ScalarGlobalCos,
    ScalarGlobalSin,
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
        use TermType::{
            PositionX, PositionY, ScalarDistance, ScalarGlobalCos, ScalarGlobalSin, ScalarRadius,
        };
        match self.t {
            ScalarDistance => write!(f, "d{}", self.base),
            PositionX => write!(f, "x{}", self.base),
            PositionY => write!(f, "y{}", self.base),
            ScalarRadius => write!(f, "r{}", self.base),
            ScalarGlobalCos => write!(f, "c{}", self.base),
            ScalarGlobalSin => write!(f, "s{}", self.base),
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
    by_base: HashMap<usize, FeatureKey>,
    free: Vec<usize>,
}

impl TermAllocator {
    pub fn get_var_ref(&self, v: &eq::Variable) -> Option<TermRef> {
        match (v.as_str().get(..1), v.as_str().get(1..)) {
            (Some("d"), Some(base)) => {
                let base: usize = base.parse().ok()?;
                Some(TermRef {
                    t: TermType::ScalarDistance,
                    base,
                    for_feature: self.by_base.get(&base).copied(),
                })
            }
            (Some("x"), Some(base)) => {
                let base: usize = base.parse().ok()?;
                Some(TermRef {
                    t: TermType::PositionX,
                    base,
                    for_feature: self.by_base.get(&base).copied(),
                })
            }
            (Some("y"), Some(base)) => {
                let base: usize = base.parse().ok()?;
                Some(TermRef {
                    t: TermType::PositionY,
                    base,
                    for_feature: self.by_base.get(&base).copied(),
                })
            }
            (Some("r"), Some(base)) => {
                let base: usize = base.parse().ok()?;
                Some(TermRef {
                    t: TermType::ScalarRadius,
                    base,
                    for_feature: self.by_base.get(&base).copied(),
                })
            }
            (Some("c"), Some(base)) => {
                let base: usize = base.parse().ok()?;
                Some(TermRef {
                    t: TermType::ScalarGlobalCos,
                    base,
                    for_feature: self.by_base.get(&base).copied(),
                })
            }
            (Some("s"), Some(base)) => {
                let base: usize = base.parse().ok()?;
                Some(TermRef {
                    t: TermType::ScalarGlobalSin,
                    base,
                    for_feature: self.by_base.get(&base).copied(),
                })
            }
            _ => None,
        }
    }

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
        self.by_base.insert(base, fk);
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
            self.by_base.remove(&base);
            self.free.push(base);
        }
    }

    /// Records deletion of a constraint, so its index can be used.
    pub fn delete_constraint(&mut self, _ck: ConstraintKey) {}

    pub fn inform_new_constraint(&mut self, _ck: ConstraintKey) {}
}
