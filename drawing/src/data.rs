use crate::Feature;
use slotmap::HopSlotMap;
use std::collections::HashMap;

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

/// Data stores state about the drawing and what it is composed of.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Data {
    pub features: HopSlotMap<slotmap::DefaultKey, Feature>,
    pub vp: Viewport,

    pub selected_map: HashMap<slotmap::DefaultKey, usize>,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            features: HopSlotMap::default(),
            vp: Viewport::default(),
            selected_map: HashMap::default(),
        }
    }
}

impl Data {
    pub fn find_point_at(&self, p: egui::Pos2) -> Option<slotmap::DefaultKey> {
        for (k, v) in self.features.iter() {
            if v.bb(self).center().distance_sq(p) < 0.0001 {
                return Some(k);
            }
        }
        None
    }

    pub fn find_screen_feature(&self, hp: egui::Pos2) -> Option<(slotmap::DefaultKey, Feature)> {
        let mut closest: Option<(slotmap::DefaultKey, f32, bool)> = None;
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

    pub fn delete_feature(&mut self, k: slotmap::DefaultKey) -> bool {
        self.selected_map.remove(&k);

        match self.features.remove(k) {
            Some(_v) => {
                // Find and also remove any features dependent on what we just removed.
                let to_delete: std::collections::HashSet<slotmap::DefaultKey> = self
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

    pub fn select_feature(&mut self, feature: &slotmap::DefaultKey, select: bool) {
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

    pub fn feature_selected(&self, feature: &slotmap::DefaultKey) -> bool {
        self.selected_map.get(feature).is_some()
    }
}
