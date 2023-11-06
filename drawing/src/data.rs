use crate::{Constraint, ConstraintKey};
use crate::{Feature, FeatureKey};
use slotmap::HopSlotMap;
use std::collections::{HashMap, HashSet};

const MAX_HOVER_DISTANCE: f32 = 160.0;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Viewport {
    pub fn screen_to_point(&self, p: egui::Pos2) -> egui::Pos2 {
        egui::Pos2 {
            x: self.zoom * p.x + self.x,
            y: self.zoom * p.y + self.y,
        }
    }
    pub fn translate_point(&self, p: egui::Pos2) -> egui::Pos2 {
        egui::Pos2 {
            x: (p.x - self.x) / self.zoom,
            y: (p.y - self.y) / self.zoom,
        }
    }
    pub fn translate_rect(&self, r: egui::Rect) -> egui::Rect {
        egui::Rect {
            min: self.translate_point(r.min),
            max: self.translate_point(r.max),
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            zoom: 1.,
        }
    }
}

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
                if by_feature.contains_key(&fk) {
                    by_feature.insert(fk, HashSet::from([ck]));
                } else {
                    by_feature.get_mut(&fk).unwrap().insert(ck);
                }
            }
        }

        self.by_feature = by_feature;
    }
}

/// Data stores state about the drawing and what it is composed of.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Data {
    pub features: HopSlotMap<FeatureKey, Feature>,
    pub constraints: ConstraintData,
    pub vp: Viewport,

    pub selected_map: HashMap<FeatureKey, usize>,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            features: HopSlotMap::default(),
            constraints: ConstraintData::default(),
            vp: Viewport::default(),
            selected_map: HashMap::default(),
        }
    }
}

impl Data {
    pub fn find_point_at(&self, p: egui::Pos2) -> Option<FeatureKey> {
        for (k, v) in self.features.iter() {
            if v.bb(self).center().distance_sq(p) < 0.0001 {
                return Some(k);
            }
        }
        None
    }

    pub fn find_screen_feature(&self, hp: egui::Pos2) -> Option<(FeatureKey, Feature)> {
        let mut closest: Option<(FeatureKey, f32, bool)> = None;
        for (k, v) in self.features.iter() {
            let is_point = v.is_point();

            // Points get a head-start in terms of being considered closer, so
            // they are chosen over a line segment when hovering near the end of
            // a line segment.
            let dist = if is_point {
                v.screen_dist(self, hp, &self.vp) - (MAX_HOVER_DISTANCE / 2.)
            } else {
                v.screen_dist(self, hp, &self.vp)
            };

            if dist < MAX_HOVER_DISTANCE {
                closest = Some(
                    closest
                        .map(|c| if dist < c.1 { (k, dist, is_point) } else { c })
                        .unwrap_or((k, dist, is_point)),
                );
            }
        }

        match closest {
            Some((k, _dist, _is_point)) => Some((k, self.features.get(k).unwrap().clone())),
            None => None,
        }
    }

    pub fn delete_feature(&mut self, k: FeatureKey) -> bool {
        self.selected_map.remove(&k);

        match self.features.remove(k) {
            Some(_v) => {
                // Find and also remove any features dependent on what we just removed.
                let to_delete: std::collections::HashSet<FeatureKey> = self
                    .features
                    .iter()
                    .map(|(k2, v2)| {
                        let dependent_deleted = v2
                            .depends_on()
                            .into_iter()
                            .filter_map(|d| d.map(|d| d == k))
                            .reduce(|p, f| p || f);

                        match dependent_deleted {
                            Some(true) => Some(k2),
                            _ => None,
                        }
                    })
                    .filter_map(|d| d)
                    .collect();

                for k in to_delete {
                    self.delete_feature(k);
                }

                true
            }
            None => false,
        }
    }

    pub fn selection_delete(&mut self) {
        let elements: Vec<_> = self.selected_map.drain().map(|(k, _)| k).collect();
        for k in elements {
            self.delete_feature(k);
        }
    }

    pub fn select_feature(&mut self, feature: &FeatureKey, select: bool) {
        let currently_selected = self.selected_map.contains_key(feature);
        if currently_selected && !select {
            self.selected_map.remove(feature);
        } else if !currently_selected && select {
            let next_idx = self.selected_map.values().fold(0, |acc, x| acc.max(*x)) + 1;
            self.selected_map.insert(feature.clone(), next_idx);
        }
    }

    pub fn select_features_in_rect(&mut self, rect: egui::Rect, select: bool) {
        let keys: Vec<_> = self
            .features
            .iter()
            .filter(|(_, v)| rect.contains_rect(v.bb(self)))
            .map(|(k, _)| k)
            .collect();

        for k in keys.into_iter() {
            self.select_feature(&k, select);
        }
    }

    pub fn selection_clear(&mut self) {
        self.selected_map.clear();
    }

    pub fn feature_selected(&self, feature: &FeatureKey) -> bool {
        self.selected_map.get(feature).is_some()
    }
}
