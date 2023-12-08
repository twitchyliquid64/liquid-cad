use crate::FeatureKey;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum GroupType {
    Boundary,
    #[default]
    Interior,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub typ: GroupType,
    pub name: String,
    pub features: Vec<FeatureKey>,
}

impl Group {
    /// Serialize returns a structure suitable for serialization to disk. Any feature
    /// which maybe referenced from the current constraint must be present in fk_to_idx.
    pub fn serialize(&self, fk_to_idx: &HashMap<FeatureKey, usize>) -> Result<SerializedGroup, ()> {
        let mut features_idx = Vec::with_capacity(self.features.len());
        for fk in self.features.iter() {
            match fk_to_idx.get(fk) {
                None => return Err(()),
                Some(idx) => features_idx.push(*idx),
            }
        }

        Ok(SerializedGroup {
            typ: self.typ,
            name: self.name.clone(),
            features_idx,
        })
    }

    pub fn deserialize(
        sg: SerializedGroup,
        idx_to_fk: &HashMap<usize, FeatureKey>,
    ) -> Result<Self, ()> {
        let mut features = Vec::with_capacity(sg.features_idx.len());
        for f_idx in sg.features_idx {
            match idx_to_fk.get(&f_idx) {
                None => return Err(()),
                Some(fk) => features.push(*fk),
            }
        }

        Ok(Self {
            typ: sg.typ,
            name: sg.name.clone(),
            features,
        })
    }

    pub fn trim_feature_if_present(&mut self, fk: FeatureKey) {
        if let Some(index) = self.features.iter().position(|&k| k == fk) {
            self.features.remove(index);
        }
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SerializedGroup {
    pub typ: GroupType,
    pub name: String,
    pub features_idx: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize() {
        use slotmap::Key;
        let point_key = FeatureKey::null();

        assert_eq!(
            Group {
                typ: GroupType::Boundary,
                name: "Ye".into(),
                features: vec![point_key]
            }
            .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedGroup {
                typ: GroupType::Boundary,
                name: "Ye".into(),
                features_idx: vec![42],
                ..SerializedGroup::default()
            }),
        );
    }

    #[test]
    fn deserialize() {
        use slotmap::Key;
        let point_key = FeatureKey::null();

        assert_eq!(
            Group::deserialize(
                SerializedGroup {
                    typ: GroupType::Boundary,
                    name: "Ye".into(),
                    features_idx: vec![42],
                    ..SerializedGroup::default()
                },
                &HashMap::from([(42, point_key)])
            ),
            Ok(Group {
                typ: GroupType::Boundary,
                name: "Ye".into(),
                features: vec![point_key],
                ..Group::default()
            }),
        );
    }
}
