use crate::FeatureKey;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum GroupType {
    Boundary,
    #[default]
    #[serde(alias = "Interior")]
    Hole,
    Extrude,
    Bore,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub typ: GroupType,
    pub name: String,
    pub features: Vec<FeatureKey>,

    pub amt: Option<f64>,
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
            amt: self.amt,
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
            amt: sg.amt,
        })
    }

    pub fn trim_feature_if_present(&mut self, fk: FeatureKey) {
        if let Some(index) = self.features.iter().position(|&k| k == fk) {
            self.features.remove(index);
        }
    }

    pub fn compute_path(&self, data: &super::Data) -> Vec<kurbo::BezPath> {
        // geometry that has been emitted
        let mut remaining = self.features.clone();
        remaining.reverse();
        // completed paths
        let mut paths: Vec<kurbo::BezPath> = Vec::with_capacity(2 * self.features.len());

        let mut current: Option<(kurbo::BezPath, egui::Pos2)> = None;
        while remaining.len() > 0 {
            match current.as_ref() {
                Some((_, end_point)) => {
                    // Theres a current path, we need to find a feature that continues it,
                    // or terminate it and start a new one.
                    //
                    // Search first for a feature with a starting point of the last point,
                    // before searching for a feature with the end point of the last point.
                    let chaining_fk = remaining
                        .iter()
                        .find(|fk| {
                            match data.features.get(**fk) {
                                Some(f) => f,
                                None => {
                                    return false;
                                }
                            }
                            .start_point(data)
                                == *end_point
                        })
                        .map(|fk| (*fk, false))
                        .or_else(|| {
                            remaining
                                .iter()
                                .find(|fk| {
                                    match data.features.get(**fk) {
                                        Some(f) => f,
                                        None => {
                                            return false;
                                        }
                                    }
                                    .end_point(data)
                                        == *end_point
                                })
                                .map(|fk| (*fk, true))
                        });

                    let mut current_path = current.take().unwrap().0;
                    match chaining_fk {
                        Some((fk, is_reverse)) => {
                            let f = data.features.get(fk).unwrap();
                            if !is_reverse {
                                for el in f.bezier_path(data).elements() {
                                    current_path.push(*el);
                                }
                                current = Some((current_path, f.end_point(data)));
                            } else {
                                for el in f.bezier_path(data).reverse_subpaths().elements() {
                                    current_path.push(*el);
                                }
                                current = Some((current_path, f.start_point(data)));
                            }
                            remaining.retain(|sfk| sfk != &fk);
                        }
                        None => {
                            paths.push(current_path);
                        }
                    }
                }
                None => {
                    let f = match data.features.get(remaining.pop().unwrap()) {
                        Some(f) => f,
                        None => continue,
                    };
                    current = Some((f.bezier_path(data), f.end_point(data)));
                }
            };
        }
        if let Some(current) = current {
            paths.push(current.0);
        }

        paths.iter_mut().for_each(|p| {
            p.apply_affine(kurbo::Affine::FLIP_Y);
        });
        paths
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SerializedGroup {
    pub typ: GroupType,
    pub name: String,
    pub features_idx: Vec<usize>,
    pub amt: Option<f64>,
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
                features: vec![point_key],
                amt: None,
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
