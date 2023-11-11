use crate::{Constraint, ConstraintKey};
use crate::{Feature, FeatureKey};
use slotmap::HopSlotMap;
use std::collections::HashMap;

const MAX_HOVER_DISTANCE: f32 = 120.0;

mod viewport;
pub use viewport::Viewport;

mod constraint_data;
pub use constraint_data::ConstraintData;

mod terms;
pub use terms::{TermAllocator, TermRef};

#[derive(Clone, Debug)]
pub enum Hover {
    None,
    Feature {
        k: FeatureKey,
        feature: Feature,
    },
    Constraint {
        k: ConstraintKey,
        constraint: Constraint,
    },
}

/// Data stores state about the drawing and what it is composed of.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Data {
    pub features: HopSlotMap<FeatureKey, Feature>,
    pub constraints: ConstraintData,
    pub vp: Viewport,

    pub selected_map: HashMap<FeatureKey, usize>,

    pub terms: TermAllocator,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            features: HopSlotMap::default(),
            constraints: ConstraintData::default(),
            vp: Viewport::default(),
            selected_map: HashMap::default(),
            terms: TermAllocator::default(),
        }
    }
}

impl Data {
    /// Call when feature or constraint fields have changed,
    /// independently of the drawing space or a handled event.
    pub fn changed_in_ui(&mut self) {
        self.solve_and_apply();
    }

    fn solve_and_apply(&mut self) {
        // quick hack before we have a solver:
        let Data {
            features,
            constraints,
            ..
        } = self;
        for (k, f) in features.iter_mut() {
            for ck in constraints.by_feature(&k) {
                if let Some(Constraint::Fixed(_, _, x, y)) = constraints.get(ck) {
                    if let Feature::Point(_, px, py) = f {
                        *px = *x;
                        *py = *y;
                    }
                }
            }
        }
    }

    /// Iterates through the features.
    pub fn features_iter(&self) -> slotmap::hop::Iter<'_, FeatureKey, Feature> {
        self.features.iter()
    }

    /// Returns the mutable feature based on the given key, if known.
    pub fn feature_mut<'a>(&'a mut self, k: FeatureKey) -> Option<&'a mut Feature> {
        let Data { features, .. } = self;

        features.get_mut(k)
    }

    /// Iterates through the constraints.
    pub fn constraints_iter(&self) -> slotmap::hop::Iter<'_, ConstraintKey, Constraint> {
        self.constraints.iter()
    }

    /// Returns the mutable constraint based on the given key, if known.
    pub fn constraint_mut<'a>(&'a mut self, ck: ConstraintKey) -> Option<&'a mut Constraint> {
        self.constraints.get_mut(ck)
    }

    /// Returns the keys of constraints known to affect the given feature.
    pub fn constraints_by_feature(&self, k: &FeatureKey) -> Vec<ConstraintKey> {
        self.constraints.by_feature(k)
    }

    /// Adds a constraint, solving to update based on any affects.
    pub fn add_constraint(&mut self, c: Constraint) {
        self.constraints.add(c);
        self.solve_and_apply();
    }

    /// Removes a constraint, solving to update based on any affects.
    pub fn delete_constraint(&mut self, k: ConstraintKey) {
        self.constraints.delete(k);
        self.terms.delete_constraint(k);
        self.solve_and_apply();
    }

    pub fn move_constraint(&mut self, k: ConstraintKey, pos: egui::Pos2) {
        if let Some(Constraint::LineLength(_, fk, _, _)) = self.constraints.get(k) {
            let (a, b) = match self.features.get(*fk) {
                Some(Feature::LineSegment(_, f1, f2)) => {
                    let (a, b) = match (
                        self.features.get(*f1).unwrap(),
                        self.features.get(*f2).unwrap(),
                    ) {
                        (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                            (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                        }
                        _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                    };

                    (self.vp.translate_point(a), self.vp.translate_point(b))
                }
                _ => {
                    panic!("feature referenced in LineLength constraint was missing or not a line")
                }
            };

            if let Some(Constraint::LineLength(_, fk, _, (ref_x, ref_y))) = self.constraint_mut(k) {
                let c = a.lerp(b, 0.5);
                let mut v = c.to_vec2() - pos.to_vec2();
                let reference = egui::Vec2::angled((a - b).angle() - v.angle()) * v.length();
                *ref_x = -reference.x;
                *ref_y = reference.y;
            };
        }
    }

    /// Returns the feature key of the point exactly at the given position.
    pub fn find_point_at(&self, p: egui::Pos2) -> Option<FeatureKey> {
        for (k, v) in self.features.iter() {
            if v.bb(self).center().distance_sq(p) < 0.0001 {
                return Some(k);
            }
        }
        None
    }

    /// Returns the 'thing' the screen coordinates are hovering over, if any.
    pub fn find_screen_hover(&self, hp: egui::Pos2) -> Hover {
        match self.find_screen_feature(hp) {
            Some((k, feature)) => Hover::Feature { k, feature },
            None => match self.find_screen_constraint(hp) {
                Some((k, constraint)) => Hover::Constraint { k, constraint },
                None => Hover::None,
            },
        }
    }

    /// Returns the feature the screen coordinates are hovering over, if any.
    fn find_screen_feature(&self, hp: egui::Pos2) -> Option<(FeatureKey, Feature)> {
        let mut closest: Option<(FeatureKey, f32, bool)> = None;
        for (k, v) in self.features.iter() {
            let is_point = v.is_point();

            // Points get a head-start in terms of being considered closer, so
            // they are chosen over a line segment when hovering near the end of
            // a line segment.
            let dist = if is_point {
                v.screen_dist_sq(self, hp, &self.vp) - (MAX_HOVER_DISTANCE / 2.)
            } else {
                v.screen_dist_sq(self, hp, &self.vp)
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

    /// Returns the constraint the screen coordinates are hovering over, if any.
    fn find_screen_constraint(&self, hp: egui::Pos2) -> Option<(ConstraintKey, Constraint)> {
        let mut closest: Option<(ConstraintKey, f32)> = None;
        for (k, c) in self.constraints_iter() {
            let dist = match c.screen_dist_sq(self, hp, &self.vp) {
                Some(dist) => dist,
                None => continue,
            };

            if dist < MAX_HOVER_DISTANCE {
                closest = Some(
                    closest
                        .map(|c| if dist < c.1 { (k, dist) } else { c })
                        .unwrap_or((k, dist)),
                );
            }
        }

        match closest {
            Some((k, _dist)) => Some((k, self.constraints.get(k).unwrap().clone())),
            None => None,
        }
    }

    /// Moves the given feature to the given coordinates, and solving to update based on
    /// any side-effects of the move.
    pub fn move_feature(&mut self, k: FeatureKey, pos: egui::Pos2) {
        let did_move_something = match self.feature_mut(k) {
            Some(Feature::Point(_, x, y)) => {
                *x = pos.x;
                *y = pos.y;
                true
            }
            _ => false,
        };

        if did_move_something {
            self.solve_and_apply();
        }
    }

    /// Removes the specified feature, iteratively removing any constraints or
    /// other features which depend on a removed feature. A solve occurs
    /// if a feature was deleted, to apply any side-effects of the delete.
    pub fn delete_feature(&mut self, k: FeatureKey) -> bool {
        self.selected_map.remove(&k);

        let out = match self.features.remove(k) {
            Some(_v) => {
                // Find and remove any constraints dependent on what we just removed.
                let dependent_constraints = self.constraints.by_feature(&k);
                for c in dependent_constraints {
                    self.constraints.delete(c);
                }

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

                self.terms.delete_feature(k);
                for k in to_delete {
                    self.delete_feature(k);
                }

                true
            }
            None => false,
        };

        if out {
            self.solve_and_apply();
        }

        out
    }

    /// Deletes the currently-selected features.
    pub fn selection_delete(&mut self) {
        let elements: Vec<_> = self.selected_map.drain().map(|(k, _)| k).collect();
        for k in elements {
            self.delete_feature(k);
        }
    }

    /// Selects or de-selects the given feature.
    pub fn select_feature(&mut self, feature: &FeatureKey, select: bool) {
        let currently_selected = self.selected_map.contains_key(feature);
        if currently_selected && !select {
            self.selected_map.remove(feature);
        } else if !currently_selected && select {
            let next_idx = self.selected_map.values().fold(0, |acc, x| acc.max(*x)) + 1;
            self.selected_map.insert(feature.clone(), next_idx);
        }
    }

    /// Selects or de-selects any features wholly within the given rectangle.
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

    /// Clears the current selection.
    pub fn selection_clear(&mut self) {
        self.selected_map.clear();
    }

    /// Selects all features.
    pub fn select_all(&mut self) {
        for k in self.features.keys().collect::<Vec<_>>() {
            self.select_feature(&k, true);
        }
    }

    /// Returns true if the feature with the given key is currently selected.
    pub fn feature_selected(&self, feature: &FeatureKey) -> bool {
        self.selected_map.get(feature).is_some()
    }
}
