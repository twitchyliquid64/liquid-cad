use crate::system::{TermAllocator, TermRef, TermType};
use crate::{Constraint, ConstraintKey, SerializedConstraint};
use crate::{Feature, FeatureKey, SerializedFeature};
use slotmap::HopSlotMap;
use std::collections::HashMap;

const MAX_HOVER_DISTANCE: f32 = 120.0;

mod viewport;
pub use viewport::Viewport;

mod constraint_data;
pub use constraint_data::ConstraintData;

pub mod group;
use group::Group;

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

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SerializedDrawing {
    pub features: Vec<SerializedFeature>,
    pub constraints: Vec<SerializedConstraint>,
    pub groups: Vec<group::SerializedGroup>,
    pub viewport: Viewport,
}

/// Data stores live state about the drawing and what it is composed of.
#[derive(Clone, Debug)]
pub struct Data {
    pub features: HopSlotMap<FeatureKey, Feature>,
    pub constraints: ConstraintData,
    pub vp: Viewport,
    pub groups: Vec<Group>,

    pub selected_map: HashMap<FeatureKey, usize>,
    pub selected_constraint: Option<ConstraintKey>,

    pub terms: TermAllocator,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            features: HopSlotMap::default(),
            constraints: ConstraintData::default(),
            vp: Viewport::default(),
            groups: vec![],
            selected_map: HashMap::default(),
            selected_constraint: None,
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
        let equations: Vec<eq::Expression> = self
            .constraints
            .iter()
            .map(|(_ck, c)| c.clone())
            .collect::<Vec<Constraint>>()
            .iter()
            .map(|c| c.equations(self))
            .flatten()
            .collect();

        if equations.len() == 0 {
            return;
        }

        // println!("Inputs:");
        // for eq in equations.iter() {
        //     println!(" - {}", eq);
        // }

        let mut solver = eq::solve::SubSolver::default();
        let mut sub_solver_state = match eq::solve::SubSolverState::new(HashMap::new(), equations) {
            Ok(st) => st,
            Err(e) => {
                println!("failed to build substitution solver: {:?}", e);
                return;
            }
        };
        // Solve as many as possible using substitution.
        let (known, unresolved) = solver.all_concrete_results(&mut sub_solver_state);
        for (v, f) in known.iter() {
            let term = self.terms.get_var_ref(v).expect("no such var");
            self.apply_solved(&term, f.as_f64());
        }

        // Solve the rest using an iterative solver.
        let residuals = solver.all_residuals(&mut sub_solver_state);
        if residuals.len() == 0 {
            return;
        }
        let initials = unresolved
            .iter()
            .map(|v| {
                let term = self.terms.get_var_ref(v).expect("no such var");
                match self.term_current_value(&term) {
                    Some(v) => v as f64,
                    None => 0.47,
                }
            })
            .collect();
        let mut solver_state = eq::solve::DumbassSolverState::new(known, unresolved, residuals);
        // println!("solver input: {:?}", solver_state);
        let mut solver = eq::solve::DumbassSolver::new_with_initials(&solver_state, initials);
        let results = match solver.solve(&mut solver_state) {
            Ok(results) => Some(results),
            Err((avg_err, results)) => {
                println!("solve failed: {}", avg_err);
                if avg_err < 24.0 {
                    Some(results)
                } else {
                    None
                }
            }
        };

        if let Some(results) = results {
            for (v, f) in results {
                let term = self.terms.get_var_ref(&v).expect("no such var");
                self.apply_solved(&term, f);
            }
        }
    }

    fn term_current_value(&self, term: &TermRef) -> Option<f32> {
        if let Some(feature) = term.for_feature {
            match self.features.get(feature) {
                Some(Feature::Point(_, x, y)) => match term.t {
                    TermType::PositionX => Some(*x),
                    TermType::PositionY => Some(*y),
                    TermType::ScalarDistance => None,
                },
                Some(Feature::LineSegment(_, f1, f2)) => match term.t {
                    TermType::ScalarDistance => {
                        let (a, b) = match (
                            self.features.get(*f1).unwrap(),
                            self.features.get(*f2).unwrap(),
                        ) {
                            (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                                (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                            }
                            _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                        };

                        Some(a.distance(b))
                    }
                    TermType::PositionX => unreachable!(),
                    TermType::PositionY => unreachable!(),
                },
                _ => None,
            }
        } else {
            None
        }
    }

    fn apply_solved(&mut self, term: &TermRef, v: f64) -> bool {
        if v.is_nan() || v.is_infinite() {
            return false;
        }

        if let Some(feature) = term.for_feature {
            match self.features.get_mut(feature) {
                Some(Feature::Point(_, x, y)) => {
                    match term.t {
                        TermType::PositionX => *x = v as f32,
                        TermType::PositionY => *y = v as f32,
                        TermType::ScalarDistance => unreachable!(),
                    }
                    true
                }
                Some(Feature::LineSegment(_, _, _)) => {
                    match term.t {
                        TermType::PositionX => unreachable!(),
                        TermType::PositionY => unreachable!(),
                        TermType::ScalarDistance => {}
                    }
                    false
                }
                _ => false,
            }
        } else {
            false
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

    pub fn feature_exists(&self, f: &Feature) -> bool {
        for v in self.features.values() {
            if v == f {
                return true;
            }
        }
        false
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
        if self.add_constraint_impl(c) {
            self.solve_and_apply();
        }
    }
    fn add_constraint_impl(&mut self, c: Constraint) -> bool {
        if let Some(ck) = self.constraints.add(c) {
            self.terms.inform_new_constraint(ck);
            true
        } else {
            false
        }
    }

    /// Removes a constraint, solving to update based on any affects.
    pub fn delete_constraint(&mut self, k: ConstraintKey) {
        self.constraints.delete(k);
        self.terms.delete_constraint(k);
        self.solve_and_apply();
    }

    /// NOTE: Only supports LineLength constraints atm, and consumes a SCREEN coordinate.
    pub fn move_constraint(&mut self, k: ConstraintKey, pos: egui::Pos2) {
        if let Some(Constraint::LineLength(_, fk, ..)) = self.constraints.get(k) {
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

            if let Some(Constraint::LineLength(_, _fk, _, _, dd)) = self.constraint_mut(k) {
                let c = a.lerp(b, 0.5);
                let v = c.to_vec2() - pos.to_vec2();
                let reference = egui::Vec2::angled((a - b).angle() - v.angle()) * v.length();
                dd.x = -reference.x;
                dd.y = reference.y;
            };
        }
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

    /// Returns the line between the two specified points, if any.
    pub fn find_line_between(&self, p1: &FeatureKey, p2: &FeatureKey) -> Option<FeatureKey> {
        self.features
            .iter()
            .filter_map(|(fk, f)| match f {
                Feature::LineSegment(_, lp1, lp2, ..) => {
                    if (lp1 == p1 && lp2 == p2) || (lp2 == p1 && lp1 == p2) {
                        Some(fk)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .next()
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
        let out = self.delete_feature_impl(k);
        if out {
            self.solve_and_apply();
        }
        out
    }

    fn delete_feature_impl(&mut self, k: FeatureKey) -> bool {
        self.selected_map.remove(&k);
        for g in self.groups.iter_mut() {
            g.trim_feature_if_present(k);
        }

        match self.features.remove(k) {
            Some(_v) => {
                // Find and remove any constraints dependent on what we just removed.
                let dependent_constraints = self.constraints.by_feature(&k);
                for c in dependent_constraints {
                    self.constraints.delete(c);
                    self.terms.delete_constraint(c);
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
                    self.delete_feature_impl(k);
                }

                true
            }
            None => false,
        }
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

    pub fn serialize(&self) -> SerializedDrawing {
        // First pass just get points
        let mut feature_keys = HashMap::with_capacity(self.features.len());
        let mut features: Vec<SerializedFeature> = self
            .features
            .iter()
            .filter(|(_fk, f)| matches!(f, Feature::Point(..)))
            .map(|(fk, f)| {
                feature_keys.insert(fk, feature_keys.len());
                f.serialize(&feature_keys).unwrap()
            })
            .collect();

        features.reserve(self.features.len());

        // Second pass gets non-points
        for (fk, f) in self.features.iter() {
            if feature_keys.contains_key(&fk) {
                continue;
            }
            feature_keys.insert(fk, feature_keys.len());
            features.push(f.serialize(&feature_keys).unwrap());
        }

        SerializedDrawing {
            features,
            constraints: self
                .constraints
                .iter()
                .map(|(_ck, c)| c.serialize(&feature_keys).unwrap())
                .collect(),
            groups: self
                .groups
                .iter()
                .map(|g| g.serialize(&feature_keys).unwrap())
                .collect(),
            viewport: self.vp.clone(),
        }
    }

    pub fn load(&mut self, drawing: SerializedDrawing) -> Result<(), ()> {
        self.features = HopSlotMap::default();
        self.constraints = ConstraintData::default();
        self.vp = drawing.viewport;

        let mut feature_keys = HashMap::with_capacity(drawing.features.len());

        for (i, sf) in drawing.features.into_iter().enumerate() {
            let fk = self
                .features
                .insert(Feature::deserialize(sf, &feature_keys).unwrap());
            feature_keys.insert(i, fk);
        }
        for sc in drawing.constraints.into_iter() {
            self.add_constraint_impl(Constraint::deserialize(sc, &feature_keys).unwrap());
        }

        self.groups = drawing
            .groups
            .into_iter()
            .map(|sg| Group::deserialize(sg, &feature_keys).unwrap())
            .collect();

        // println!("features: {:?}", self.features);
        // println!("constraints: {:?}", self.constraints);
        self.solve_and_apply();
        Ok(())
    }

    pub fn serialize_openscad(&self, flatten_tolerance: f64) -> Result<String, ()> {
        let (points, idx_outer, idx_inner) = self.flatten_to_idxs(flatten_tolerance)?;
        if idx_outer.len() > 1 {
            return Err(());
        }

        let mut out: String = String::from("polygon(\n  points = [\n    ");
        out.reserve(64 + points.len() * 10 + idx_outer.len() * 5 + idx_inner.len() * 5);

        let points_len = points.len();
        for (i, point) in points.into_iter().enumerate() {
            if i % 8 == 0 && i > 0 {
                out.push_str("\n    ");
            }
            out.push_str("[");
            out.push_str(&format!("{}, {}", point.x, point.y).to_string());
            out.push_str("]");
            if i + 1 < points_len {
                out.push_str(", ");
            }
        }
        out.push_str("\n  ],\n");

        out.push_str("  paths = [");

        let outer_len = idx_outer.len();
        for (i, path) in idx_outer.into_iter().enumerate() {
            out.push_str("\n    [");
            let path_len = path.len();
            for (j, idx) in path.into_iter().enumerate() {
                out.push_str(&format!("{}", idx).to_string());
                if j + 1 < path_len {
                    out.push_str(", ");
                }
            }
            out.push_str("]");
            if idx_inner.len() > 0 || i + 1 < outer_len {
                out.push_str(",");
            }
        }
        let inner_len = idx_inner.len();
        for (i, path) in idx_inner.into_iter().enumerate() {
            out.push_str("\n    [");
            let path_len = path.len();
            for (j, idx) in path.into_iter().enumerate() {
                out.push_str(&format!("{}", idx).to_string());
                if j + 1 < path_len {
                    out.push_str(", ");
                }
            }
            out.push_str("]");
            if i + 1 < inner_len {
                out.push_str(",");
            }
        }

        out.push_str("\n  ],\n  ");
        out.push_str("convexity = 10\n);");

        Ok(out)
    }

    pub fn flatten_to_idxs(
        &self,
        flatten_tolerance: f64,
    ) -> Result<(Vec<kurbo::Point>, Vec<Vec<usize>>, Vec<Vec<usize>>), ()> {
        use crate::GroupType;
        let mut points: Vec<kurbo::Point> = Vec::with_capacity(128);
        let mut indices_outer: Vec<Vec<usize>> = Vec::with_capacity(2);
        let mut indices_inner: Vec<Vec<usize>> = Vec::with_capacity(6);

        let paths: Vec<(GroupType, Result<Vec<Vec<kurbo::Point>>, ()>)> = self
            .groups
            .iter()
            .map(|g| {
                let out = if let Ok(paths) = g.compute_path(self) {
                    let mut out_paths: Vec<Vec<kurbo::Point>> = Vec::with_capacity(4);
                    for path in paths.into_iter() {
                        let mut points: Vec<kurbo::Point> = Vec::with_capacity(32);
                        path.flatten(flatten_tolerance, |el| {
                            use kurbo::PathEl;
                            match el {
                                PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                                    if points.len() == 0 || points[points.len() - 1] != p {
                                        points.push(p);
                                    }
                                }
                                PathEl::ClosePath => {}
                                _ => panic!("unexpected element: {:?}", el),
                            }
                        });
                        if points.len() > 0 {
                            out_paths.push(points);
                        }
                    }
                    Ok(out_paths)
                } else {
                    Err(())
                };

                (g.typ, out)
            })
            .collect();

        // Do boundaries first
        for path_points in paths
            .iter()
            .filter(|(gt, _)| gt == &GroupType::Boundary)
            .map(|(_gt, paths)| paths.iter())
            .flatten()
            .flatten()
        {
            let mut idx: Vec<usize> = Vec::with_capacity(path_points.len());
            for point in path_points.iter() {
                points.push(*point);
                idx.push(points.len() - 1);
            }
            indices_outer.push(idx);
        }
        // Now interior geometry
        for path_points in paths
            .iter()
            .filter(|(gt, _)| gt == &GroupType::Interior)
            .map(|(_gt, paths)| paths.iter())
            .flatten()
            .flatten()
        {
            let mut idx: Vec<usize> = Vec::with_capacity(path_points.len());
            for point in path_points.iter() {
                points.push(*point);
                idx.push(points.len() - 1);
            }
            indices_inner.push(idx);
        }

        Ok((points, indices_outer, indices_inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, ConstraintMeta, DimensionDisplay, SerializedConstraint};
    use crate::{FeatureMeta, SerializedFeature};

    #[test]
    fn serialize_features() {
        let mut data = Data::default();
        let p1 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 0.0, 0.0));
        let p2 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 5.0, 0.0));
        data.features
            .insert(Feature::LineSegment(FeatureMeta::default(), p1, p2));
        let p3 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 2.5, 0.0));
        data.features
            .insert(Feature::Arc(FeatureMeta::default(), p1, p3, p2));

        assert_eq!(
            data.serialize().features,
            vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    meta: FeatureMeta::default(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    meta: FeatureMeta::default(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    meta: FeatureMeta::default(),
                    using_idx: vec![],
                    x: 2.5,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    meta: FeatureMeta::default(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "arc".to_string(),
                    meta: FeatureMeta::default(),
                    using_idx: vec![0, 2, 1],
                    ..SerializedFeature::default()
                },
            ],
        );
    }

    #[test]
    fn serialize_constraints() {
        let mut data = Data::default();
        let p1 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 0.0, 0.0));
        let p2 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 5.0, 0.0));
        let line1 = data
            .features
            .insert(Feature::LineSegment(FeatureMeta::default(), p1, p2));
        let p3 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 5.0, -5.0));
        let line2 = data
            .features
            .insert(Feature::LineSegment(FeatureMeta::default(), p2, p3));

        data.add_constraint(Constraint::Fixed(ConstraintMeta::default(), p1, 0., 0.));
        data.add_constraint(Constraint::LineLength(
            ConstraintMeta::default(),
            line2,
            5.0,
            Some((Axis::TopBottom, true)),
            DimensionDisplay::default(),
        ));
        data.add_constraint(Constraint::LineLengthsEqual(
            ConstraintMeta::default(),
            line1,
            line2,
            None,
        ));

        assert_eq!(
            data.serialize(),
            SerializedDrawing {
                features: vec![
                    SerializedFeature {
                        kind: "pt".to_string(),
                        meta: FeatureMeta::default(),
                        using_idx: vec![],
                        x: 0.0,
                        y: 0.0,
                        ..SerializedFeature::default()
                    },
                    SerializedFeature {
                        kind: "pt".to_string(),
                        meta: FeatureMeta::default(),
                        using_idx: vec![],
                        x: 5.0,
                        y: 0.0,
                        ..SerializedFeature::default()
                    },
                    SerializedFeature {
                        kind: "pt".to_string(),
                        meta: FeatureMeta::default(),
                        using_idx: vec![],
                        x: 5.0,
                        y: -5.0,
                        ..SerializedFeature::default()
                    },
                    SerializedFeature {
                        kind: "line".to_string(),
                        meta: FeatureMeta::default(),
                        using_idx: vec![0, 1],
                        ..SerializedFeature::default()
                    },
                    SerializedFeature {
                        kind: "line".to_string(),
                        meta: FeatureMeta::default(),
                        using_idx: vec![1, 2],
                        ..SerializedFeature::default()
                    },
                ],
                constraints: vec![
                    SerializedConstraint {
                        kind: "fixed".to_string(),
                        at: (0.0, 0.0),
                        feature_idx: vec![0],
                        ..SerializedConstraint::default()
                    },
                    SerializedConstraint {
                        kind: "length".to_string(),
                        feature_idx: vec![4],
                        amt: 5.0,
                        cardinality: Some((Axis::TopBottom, true)),
                        ..SerializedConstraint::default()
                    },
                    SerializedConstraint {
                        kind: "line_lengths_equal".to_string(),
                        feature_idx: vec![3, 4],
                        ..SerializedConstraint::default()
                    }
                ],
                ..SerializedDrawing::default()
            }
        );
    }

    #[test]
    fn serialize_groups() {
        let mut data = Data::default();
        let p1 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 0.0, 0.0));
        let p2 = data
            .features
            .insert(Feature::Point(FeatureMeta::default(), 5.0, 0.0));
        let l1 = data
            .features
            .insert(Feature::LineSegment(FeatureMeta::default(), p1, p2));

        data.groups = vec![Group {
            typ: group::GroupType::Boundary,
            name: "yolo".into(),
            features: vec![p1, p2, l1],
        }];

        assert_eq!(
            data.serialize().groups,
            vec![group::SerializedGroup {
                typ: group::GroupType::Boundary,
                name: "yolo".into(),
                features_idx: vec![0, 1, 2],
            },],
        );
    }

    #[test]
    fn load_basic() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (0.0, 0.0),
                    feature_idx: vec![0],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "length".to_string(),
                    feature_idx: vec![2],
                    amt: 15.0,
                    cardinality: Some((Axis::LeftRight, true)),
                    ..SerializedConstraint::default()
                },
            ],
            groups: vec![group::SerializedGroup {
                typ: group::GroupType::Interior,
                name: "yeet".into(),
                features_idx: vec![0, 1, 2],
                ..group::SerializedGroup::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        // So we loaded two points, with a line that constrained the
        // second point such that it was at (-15, 0). Lets test
        // that was solved.
        assert_eq!(
            data.features_iter().map(|(_fk, f)| f).nth(1),
            Some(Feature::Point(FeatureMeta::default(), -15.0, 0.0,)).as_ref()
        );

        // Make sure that group exists too
        assert_eq!(
            data.groups,
            vec![Group {
                typ: group::GroupType::Interior,
                name: "yeet".into(),
                features: vec![
                    data.features_iter().map(|(fk, _f)| fk).nth(0).unwrap(),
                    data.features_iter().map(|(fk, _f)| fk).nth(1).unwrap(),
                    data.features_iter().map(|(fk, _f)| fk).nth(2).unwrap(),
                ],
                ..Group::default()
            },],
        );
    }

    #[test]
    fn solve_eqidistant() {
        //        p1
        // d=14 /   \ d=14
        //     /     \
        //   p0       p2
        // (-5, 0)  (5, 0)

        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: -5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 10.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![2, 1],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (-5.0, 0.0),
                    feature_idx: vec![0],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (5.0, 0.0),
                    feature_idx: vec![2],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "length".to_string(),
                    feature_idx: vec![3],
                    amt: 14.0,
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "length".to_string(),
                    feature_idx: vec![4],
                    amt: 14.0,
                    ..SerializedConstraint::default()
                },
            ],
            ..SerializedDrawing::default()
        })
        .unwrap();

        let point = data.features_iter().map(|(_fk, f)| f).nth(1).unwrap();
        assert!(matches!(point, Feature::Point(_, x, y) if x.abs() < 0.005 && y > &11.0 ));
    }

    #[test]
    fn solve_parallel() {
        //        p1 (3, 4)  p3
        //      /           /
        //     /           /
        //   p0          p2
        // (0, 0)    (10, 0)

        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 3.0,
                    y: 4.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 10.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![2, 3],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (0.0, 0.0),
                    feature_idx: vec![0],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (3.0, 4.0),
                    feature_idx: vec![1],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (10.0, 0.0),
                    feature_idx: vec![2],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "length".to_string(),
                    feature_idx: vec![4],
                    amt: 5.0,
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "length".to_string(),
                    feature_idx: vec![5],
                    amt: 5.0,
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "lines_parallel".to_string(),
                    feature_idx: vec![4, 5],
                    ..SerializedConstraint::default()
                },
            ],
            ..SerializedDrawing::default()
        })
        .unwrap();

        let point = data.features_iter().map(|(_fk, f)| f).nth(3).unwrap();
        assert!(
            matches!(point, Feature::Point(_, x, y) if (13.0 - x).abs() < 0.005 && (4.0 - y).abs() < 0.005 )
        );
    }

    #[test]
    fn solve_line_lengths_ratio() {
        //   p0 ----- p1
        // (0, 0)  (5, 0)
        //   |
        //   | d = 2.0 * d(p0, p1)
        //   |
        //  p2

        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: -5.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 2],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (0.0, 0.0),
                    feature_idx: vec![0],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    at: (5.0, 0.0),
                    feature_idx: vec![1],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "vertical".to_string(),
                    feature_idx: vec![4],
                    ..SerializedConstraint::default()
                },
                SerializedConstraint {
                    kind: "line_lengths_equal".to_string(),
                    feature_idx: vec![3, 4],
                    amt: 2.0,
                    ..SerializedConstraint::default()
                },
            ],
            ..SerializedDrawing::default()
        })
        .unwrap();

        let point = data.features_iter().map(|(_fk, f)| f).nth(2).unwrap();
        assert!(
            matches!(point, Feature::Point(_, x, y) if x.abs() < 0.005 && (10.0 + y).abs() < 0.05 )
        );
    }

    #[test]
    fn feature_also_deleted_from_group() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![SerializedFeature {
                kind: "pt".to_string(),
                using_idx: vec![],
                ..SerializedFeature::default()
            }],
            groups: vec![group::SerializedGroup {
                typ: group::GroupType::Interior,
                name: "yeet".into(),
                features_idx: vec![0],
                ..group::SerializedGroup::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        data.delete_feature(data.features_iter().map(|(fk, _f)| fk).nth(0).unwrap());

        // Make sure that group no longer has any features
        assert_eq!(
            data.groups,
            vec![Group {
                typ: group::GroupType::Interior,
                name: "yeet".into(),
                features: vec![],
                ..Group::default()
            },],
        );
    }

    #[test]
    fn new_arc_constrains_midpoint() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
            ],
            ..SerializedDrawing::default()
        })
        .unwrap();

        // Simulate creating an Arc with the Arc tool
        let (pt1, pt2) = (
            data.features_iter().map(|(fk, _f)| fk).nth(0).unwrap(),
            data.features_iter().map(|(fk, _f)| fk).nth(1).unwrap(),
        );
        let mut tools = crate::tools::Toolbar::default();
        crate::Handler::default().handle(
            &mut data,
            &mut tools,
            crate::handler::ToolResponse::NewArc(pt1, pt2),
        );

        // See if we now have a constraint that applies to the new midpoint,
        // lerp'ing it to the midpoint of the line between
        assert!(matches!(
            data.constraints.iter().next().unwrap().1,
            Constraint::PointLerpLine(_, _l_fk, mid_fk, amt)
                if mid_fk == &data.features_iter().map(|(fk, _f)| fk).nth(2).unwrap() &&
                *amt == 0.5,
        ));
    }

    #[test]
    fn applying_horizontal_sets_line_length_cardinality_positive() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![SerializedConstraint {
                kind: "length".to_string(),
                feature_idx: vec![2],
                amt: 5.0,
                ..SerializedConstraint::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        // Simulate creating a horizontal constraint
        let line_fk = data.features_iter().map(|(fk, _f)| fk).nth(2).unwrap();
        let mut tools = crate::tools::Toolbar::default();
        crate::Handler::default().handle(
            &mut data,
            &mut tools,
            crate::handler::ToolResponse::NewLineCardinalConstraint(line_fk, true), // true = horizontal
        );

        // Make sure that line length constraint got updated with an axis
        assert!(matches!(
            data.constraints.iter().next().unwrap().1,
            Constraint::LineLength(_, c_fk, _amt, Some((Axis::LeftRight, false)), ..)
                if c_fk == &line_fk,
        ));
    }

    #[test]
    fn applying_horizontal_sets_line_length_cardinality_negative() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![1, 0],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![SerializedConstraint {
                kind: "length".to_string(),
                feature_idx: vec![2],
                amt: 5.0,
                ..SerializedConstraint::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        // Simulate creating a horizontal constraint
        let line_fk = data.features_iter().map(|(fk, _f)| fk).nth(2).unwrap();
        let mut tools = crate::tools::Toolbar::default();
        crate::Handler::default().handle(
            &mut data,
            &mut tools,
            crate::handler::ToolResponse::NewLineCardinalConstraint(line_fk, true), // true = horizontal
        );

        // Make sure that line length constraint got updated with an axis
        assert!(matches!(
            data.constraints.iter().next().unwrap().1,
            Constraint::LineLength(_, c_fk, _amt, Some((Axis::LeftRight, true)), ..)
                if c_fk == &line_fk,
        ));
    }

    #[test]
    fn applying_line_length_to_horizontal_sets_cardinality_positive() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![SerializedConstraint {
                kind: "horizontal".to_string(),
                feature_idx: vec![2],
                ..SerializedConstraint::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        // Simulate creating a line length constraint
        let line_fk = data.features_iter().map(|(fk, _f)| fk).nth(2).unwrap();
        let mut tools = crate::tools::Toolbar::default();
        crate::Handler::default().handle(
            &mut data,
            &mut tools,
            crate::handler::ToolResponse::NewLineLengthConstraint(line_fk),
        );

        // Make sure that the only constraint is the line length constraint we want
        assert!(data.constraints.iter().len() == 1);
        assert!(matches!(
            data.constraints.iter().next().unwrap().1,
            Constraint::LineLength(_, c_fk, amt, Some((Axis::LeftRight, false)), ..)
                if c_fk == &line_fk && *amt == 5.0,
        ));
    }

    #[test]
    fn applying_line_length_to_horizontal_sets_cardinality_negative() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: -5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
            ],
            constraints: vec![SerializedConstraint {
                kind: "horizontal".to_string(),
                feature_idx: vec![2],
                ..SerializedConstraint::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        // Simulate creating a line length constraint
        let line_fk = data.features_iter().map(|(fk, _f)| fk).nth(2).unwrap();
        let mut tools = crate::tools::Toolbar::default();
        crate::Handler::default().handle(
            &mut data,
            &mut tools,
            crate::handler::ToolResponse::NewLineLengthConstraint(line_fk),
        );

        // Make sure that the only constraint is the line length constraint we want
        assert!(data.constraints.iter().len() == 1);
        assert!(matches!(
            data.constraints.iter().next().unwrap().1,
            Constraint::LineLength(_, c_fk, amt, Some((Axis::LeftRight, true)), ..)
                if c_fk == &line_fk && *amt == 5.0,
        ));
    }

    #[test]
    fn compute_path_group_basic_lines() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 5.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![1, 3],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![3, 0],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 15.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 15.0,
                    y: 15.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![6, 7],
                    ..SerializedFeature::default()
                },
            ],
            groups: vec![crate::SerializedGroup {
                typ: crate::GroupType::Boundary,
                name: "Ye".into(),
                features_idx: vec![2, 4, 5, 8],
                ..crate::SerializedGroup::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        assert_eq!(
            data.groups[0].compute_path(&data),
            Ok(vec![
                kurbo::BezPath::from_vec(vec![
                    kurbo::PathEl::MoveTo(kurbo::Point { x: 0.0, y: 0.0 }),
                    kurbo::PathEl::LineTo(kurbo::Point { x: 5.0, y: 0.0 }),
                    kurbo::PathEl::MoveTo(kurbo::Point { x: 5.0, y: 0.0 }),
                    kurbo::PathEl::LineTo(kurbo::Point { x: 5.0, y: 5.0 }),
                    kurbo::PathEl::MoveTo(kurbo::Point { x: 5.0, y: 5.0 }),
                    kurbo::PathEl::LineTo(kurbo::Point { x: 0.0, y: 0.0 }),
                ]),
                kurbo::BezPath::from_vec(vec![
                    kurbo::PathEl::MoveTo(kurbo::Point { x: 0.0, y: 15.0 }),
                    kurbo::PathEl::LineTo(kurbo::Point { x: 15.0, y: 15.0 }),
                ]),
            ])
        );
    }

    #[test]
    fn compute_path_group_line_arc_circle() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 1.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 4.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "arc".to_string(),
                    using_idx: vec![1, 4, 3],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 4.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![3, 6],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "circle".to_string(),
                    using_idx: vec![1],
                    r: 5.0,
                    ..SerializedFeature::default()
                },
            ],
            groups: vec![crate::SerializedGroup {
                typ: crate::GroupType::Boundary,
                name: "Ye".into(),
                features_idx: vec![2, 5, 7, 8],
                ..crate::SerializedGroup::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        let flattened = data.groups[0].compute_path(&data).unwrap();
        //println!("{:?}", flattened);

        assert_eq!(
            &flattened[0].elements()[0..3],
            &[
                kurbo::PathEl::MoveTo(kurbo::Point { x: 0.0, y: 0.0 }),
                kurbo::PathEl::LineTo(kurbo::Point { x: 5.0, y: 0.0 }),
                kurbo::PathEl::MoveTo(kurbo::Point { x: 5.0, y: 0.0 }),
            ],
        );
        assert!(matches!(
            &flattened[0].elements()[3],
            kurbo::PathEl::CurveTo(_, _, end) if end == &kurbo::Point { x: 5.0, y: 1.0 },
        ));
        assert_eq!(
            &flattened[0].elements()[4..],
            &[
                kurbo::PathEl::MoveTo(kurbo::Point { x: 5.0, y: 1.0 }),
                kurbo::PathEl::LineTo(kurbo::Point { x: 5.0, y: 4.0 }),
            ],
        );

        // Circle
        assert_eq!(
            &flattened[1].elements()[..1],
            &[kurbo::PathEl::MoveTo(kurbo::Point { x: 10.0, y: 0.0 }),],
        );
        assert!(matches!(
            &flattened[1].elements()[1],
            kurbo::PathEl::CurveTo(_, _, end) if end == &kurbo::Point { x: 5.0, y: 5.0 },
        ));
        assert!(matches!(
            &flattened[1].elements()[4],
            kurbo::PathEl::CurveTo(_, _, end) if end == &kurbo::Point { x: 10.0, y: 0.0 },
        ));
    }

    #[test]
    fn flatten_to_idxs() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 5.0,
                    y: 5.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 1],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![1, 2],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![2, 0],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 4.0,
                    y: 2.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 4.0,
                    y: 3.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![0, 6],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![6, 7],
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "line".to_string(),
                    using_idx: vec![7, 0],
                    ..SerializedFeature::default()
                },
            ],
            groups: vec![
                crate::SerializedGroup {
                    typ: crate::GroupType::Boundary,
                    name: "Ye".into(),
                    features_idx: vec![3, 4, 5],
                    ..crate::SerializedGroup::default()
                },
                crate::SerializedGroup {
                    typ: crate::GroupType::Interior,
                    name: "Cutout".into(),
                    features_idx: vec![8, 9, 10],
                    ..crate::SerializedGroup::default()
                },
            ],
            ..SerializedDrawing::default()
        })
        .unwrap();

        let (points, idx_outer, idx_inner) = data.flatten_to_idxs(5.0).unwrap();
        assert_eq!(
            points,
            vec![
                kurbo::Point { x: 0.0, y: 0.0 },
                kurbo::Point { x: 5.0, y: 0.0 },
                kurbo::Point { x: 5.0, y: 5.0 },
                kurbo::Point { x: 0.0, y: 0.0 },
                kurbo::Point { x: 0.0, y: 0.0 },
                kurbo::Point { x: 4.0, y: 2.0 },
                kurbo::Point { x: 4.0, y: 3.0 },
                kurbo::Point { x: 0.0, y: 0.0 },
            ],
        );

        assert_eq!(idx_outer, vec![vec![0, 1, 2, 3]]);
        assert_eq!(idx_inner, vec![vec![4, 5, 6, 7]]);
        // println!("{}", data.serialize_openscad(5.0).unwrap());
        assert_eq!(
            data.serialize_openscad(5.0).unwrap().as_str(),
            "polygon(
  points = [
    [0, 0], [5, 0], [5, 5], [0, 0], [0, 0], [4, 2], [4, 3], [0, 0]
  ],
  paths = [
    [0, 1, 2, 3],
    [4, 5, 6, 7]
  ],
  convexity = 10
);"
        );
    }

    #[test]
    fn flatten_to_idxs_circle() {
        let mut data = Data::default();
        data.load(SerializedDrawing {
            features: vec![
                SerializedFeature {
                    kind: "pt".to_string(),
                    using_idx: vec![],
                    x: 0.0,
                    y: 0.0,
                    ..SerializedFeature::default()
                },
                SerializedFeature {
                    kind: "circle".to_string(),
                    using_idx: vec![0],
                    r: 2.0,
                    ..SerializedFeature::default()
                },
            ],
            groups: vec![crate::SerializedGroup {
                typ: crate::GroupType::Boundary,
                name: "Ye".into(),
                features_idx: vec![1],
                ..crate::SerializedGroup::default()
            }],
            ..SerializedDrawing::default()
        })
        .unwrap();

        let (points, idx_outer, idx_inner) = data.flatten_to_idxs(1.0).unwrap();
        assert_eq!(points.len(), 5);
        assert_eq!(points[0], kurbo::Point { x: 2.0, y: 0.0 });
        assert_eq!(points[1].y, 2.0);
        assert_eq!(points[3].y, -2.0);
        assert_eq!(points[4], kurbo::Point { x: 2.0, y: 0.0 });

        assert_eq!(idx_outer, vec![vec![0, 1, 2, 3, 4]]);
        assert_eq!(idx_inner, Vec::<Vec<usize>>::new());
    }
}
