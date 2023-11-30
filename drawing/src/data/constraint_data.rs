use crate::FeatureKey;
use crate::{Constraint, ConstraintKey};
use slotmap::HopSlotMap;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct ConstraintData {
    constraints: HopSlotMap<ConstraintKey, Constraint>,

    #[serde(skip)]
    by_feature: HashMap<FeatureKey, HashSet<ConstraintKey>>,
}

impl ConstraintData {
    pub fn populate_cache(&mut self) {
        let mut by_feature = HashMap::with_capacity(2 * self.constraints.len());
        for (ck, c) in self.constraints.iter() {
            for fk in c.affecting_features() {
                if !by_feature.contains_key(&fk) {
                    by_feature.insert(fk, HashSet::from([ck]));
                } else {
                    by_feature.get_mut(&fk).unwrap().insert(ck);
                }
            }
        }

        self.by_feature = by_feature;
    }

    pub fn iter(&self) -> slotmap::hop::Iter<'_, ConstraintKey, Constraint> {
        self.constraints.iter()
    }

    pub fn add(&mut self, c: Constraint) -> Option<ConstraintKey> {
        for c2 in self.constraints.values() {
            if c.conflicts(c2) {
                return None;
            }
        }

        let k = self.constraints.insert(c.clone());
        for fk in c.affecting_features() {
            if !self.by_feature.contains_key(&fk) {
                self.by_feature.insert(fk, HashSet::from([k]));
            } else {
                self.by_feature.get_mut(&fk).unwrap().insert(k);
            }
        }
        Some(k)
    }

    pub fn delete(&mut self, ck: ConstraintKey) {
        match self.constraints.remove(ck) {
            Some(c) => {
                for fk in c.affecting_features() {
                    let remaining_entries = if let Some(set) = self.by_feature.get_mut(&fk) {
                        set.remove(&ck);
                        set.len()
                    } else {
                        99999
                    };

                    if remaining_entries == 0 {
                        self.by_feature.remove(&fk);
                    }
                }
            }
            None => {}
        }
    }

    pub fn by_feature(&self, k: &FeatureKey) -> Vec<ConstraintKey> {
        match self.by_feature.get(k) {
            Some(set) => set.iter().map(|ck| ck.clone()).collect(),
            None => vec![],
        }
    }

    pub fn get_mut<'a>(&'a mut self, ck: ConstraintKey) -> Option<&'a mut Constraint> {
        self.constraints.get_mut(ck)
    }
    pub fn get(&self, ck: ConstraintKey) -> Option<&Constraint> {
        self.constraints.get(ck)
    }
}
