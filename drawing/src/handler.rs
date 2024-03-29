use super::{Data, Feature, FeatureKey, FeatureMeta};
use crate::tools::Toolbar;
use crate::{Axis, Constraint, ConstraintKey, ConstraintMeta, DimensionDisplay};

#[derive(Debug)]
pub enum ToolResponse {
    Handled,
    SwitchToPointer,
    NewPoint(egui::Pos2),
    NewLineSegment(FeatureKey, FeatureKey),
    NewArc(FeatureKey, FeatureKey),
    NewCircle(FeatureKey, egui::Pos2),
    NewSpurGear(FeatureKey),
    NewRegularPoly(FeatureKey),
    Delete(FeatureKey),

    NewFixedConstraint(FeatureKey),
    NewLineLengthConstraint(FeatureKey),
    NewCircleRadiusConstraint(FeatureKey),
    NewLineCardinalConstraint(FeatureKey, bool), // true = horizontal
    NewPointLerp(FeatureKey, FeatureKey),        // point, line
    NewEqual(FeatureKey, FeatureKey),
    NewParallelLine(FeatureKey, FeatureKey),
    NewGlobalAngleConstraint(FeatureKey),

    ConstraintDelete(ConstraintKey),
    ConstraintLinesEqualRemoveMultiplier(ConstraintKey),
    ConstraintRadiusEqualRemoveMultiplier(ConstraintKey),

    DeleteGroup(usize),

    ArrayWizard(FeatureKey, egui::Vec2, crate::data::ContextMenuData),
}

#[derive(Debug, Default)]
pub struct Handler {}

impl Handler {
    pub fn handle(&mut self, drawing: &mut Data, tools: &mut Toolbar, c: ToolResponse) {
        match c {
            ToolResponse::Handled => {}
            ToolResponse::SwitchToPointer => {
                tools.clear();
            }
            ToolResponse::DeleteGroup(idx) => {
                drawing.groups.remove(idx);
            }
            ToolResponse::NewPoint(pos) => {
                let pos = drawing.vp.screen_to_point(pos);
                let p = Feature::Point(FeatureMeta::default(), pos.x, pos.y);

                if drawing.feature_exists(&p) {
                    return;
                }

                drawing.features.insert(p);
            }

            ToolResponse::NewLineSegment(p1, p2) => {
                let l = Feature::LineSegment(FeatureMeta::default(), p2, p1);

                if drawing.feature_exists(&l) {
                    return;
                }

                drawing.features.insert(l);
            }

            ToolResponse::NewArc(fk1, fk2) => {
                let (f1, f2) = (
                    drawing.features.get(fk1).unwrap(),
                    drawing.features.get(fk2).unwrap(),
                );
                let (p1, p2) = match (f1, f2) {
                    (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                        (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                    }
                    _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                };

                // Create the midpoint point.
                let mid = p1.lerp(p2, 0.5);
                let mid_fk = drawing.features.insert(Feature::Point(
                    FeatureMeta::default_construction(),
                    mid.x,
                    mid.y,
                ));

                // Create a line between the points if none exists.
                let line_fk = match drawing.find_line_between(&fk1, &fk2) {
                    Some(fk) => fk,
                    None => drawing.features.insert(Feature::LineSegment(
                        FeatureMeta::default_construction(),
                        fk1,
                        fk2,
                    )),
                };

                // Constrain the midpoint to be at the 0.5 lerp of the line.
                drawing.add_constraint(Constraint::PointLerpLine(
                    ConstraintMeta::default(),
                    line_fk,
                    mid_fk,
                    0.5,
                ));

                // Finally, create the arc feature.
                let a = Feature::Arc(FeatureMeta::default(), fk1, mid_fk, fk2);
                drawing.features.insert(a);

                tools.clear();
            }
            ToolResponse::NewCircle(center, pos) => {
                let pos = drawing.vp.screen_to_point(pos);
                let center_pos = match drawing.features.get(center) {
                    Some(Feature::Point(_, x, y, ..)) => egui::Pos2 { x: *x, y: *y },
                    _ => unreachable!(),
                };

                let p = Feature::Circle(FeatureMeta::default(), center, center_pos.distance(pos));

                if drawing.feature_exists(&p) {
                    return;
                }
                drawing.features.insert(p);
                tools.clear();
            }
            ToolResponse::NewSpurGear(p_center) => {
                let g =
                    Feature::SpurGear(FeatureMeta::default(), p_center, super::GearInfo::default());

                if drawing.feature_exists(&g) {
                    return;
                }

                drawing.features.insert(g);
                tools.clear();
            }
            ToolResponse::NewRegularPoly(p_center) => {
                let g = Feature::RegularPoly(FeatureMeta::default(), p_center, 6, 4.0);

                if drawing.feature_exists(&g) {
                    return;
                }

                drawing.features.insert(g);
                tools.clear();
            }

            ToolResponse::Delete(k) => {
                drawing.delete_feature(k);
            }
            ToolResponse::ConstraintDelete(k) => {
                drawing.delete_constraint(k);
            }

            ToolResponse::NewFixedConstraint(k) => match drawing.features.get(k) {
                Some(Feature::Point(..)) => {
                    drawing.add_constraint(Constraint::Fixed(ConstraintMeta::default(), k, 0., 0.));
                    tools.clear();
                }
                _ => {}
            },
            ToolResponse::NewLineLengthConstraint(k) => match drawing.features.get(k) {
                Some(Feature::LineSegment(_, f1, f2)) => {
                    let (f1, f2) = (
                        drawing.features.get(*f1).unwrap(),
                        drawing.features.get(*f2).unwrap(),
                    );
                    let (p1, p2) = match (f1, f2) {
                        (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                            (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                        }
                        _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                    };

                    let d = p1.distance(p2);
                    let mut cardinality: Option<(Axis, bool)> = None;

                    // If we are dimensioning a line which already has a cardinality, remove the
                    // cardinality constraint and just roll it into our length constraint.
                    for ck in drawing.constraints_by_feature(&k).into_iter() {
                        match drawing.constraints.get_mut(ck) {
                            Some(Constraint::LineAlongCardinal(_, _, axis, ..)) => {
                                cardinality = Some((
                                    axis.clone(),
                                    match axis {
                                        Axis::TopBottom => p1.y > p2.y,
                                        Axis::LeftRight => p1.x > p2.x,
                                    },
                                ));
                                drawing.delete_constraint(ck);
                            }
                            _ => {}
                        }
                    }

                    drawing.add_constraint(Constraint::LineLength(
                        ConstraintMeta::default(),
                        k,
                        d,
                        cardinality,
                        DimensionDisplay {
                            x: 0.,
                            y: 35.0,
                            ..DimensionDisplay::default()
                        },
                    ));
                    tools.clear();
                }
                _ => {}
            },
            ToolResponse::NewLineCardinalConstraint(k, is_horizontal) => {
                let want_axis = if is_horizontal {
                    Axis::LeftRight
                } else {
                    Axis::TopBottom
                };
                let (p1, p2) = match drawing.features.get(k) {
                    Some(Feature::LineSegment(_, f1, f2)) => {
                        let (f1, f2) = (
                            drawing.features.get(*f1).unwrap(),
                            drawing.features.get(*f2).unwrap(),
                        );
                        match (f1, f2) {
                            (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                                (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                            }
                            _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                        }
                    }
                    _ => unreachable!(),
                };

                // Delete any existing Cardinal constraint that were opposite
                let clashing_constraints: Vec<_> = drawing
                    .constraints_by_feature(&k)
                    .into_iter()
                    .filter_map(|ck| match drawing.constraints.get(ck) {
                        Some(Constraint::LineAlongCardinal(_, _, axis, ..)) => {
                            if axis != &want_axis {
                                Some(ck)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect();
                for ck in clashing_constraints {
                    drawing.delete_constraint(ck);
                }

                // Instead of making a new constraint, setup cardinality on a distance
                // constraint if one exists.
                for ck in drawing.constraints_by_feature(&k).into_iter() {
                    match drawing.constraints.get_mut(ck) {
                        Some(Constraint::LineLength(_, _fk, _dist, cardinality, ..)) => {
                            *cardinality = Some((
                                want_axis.clone(),
                                match want_axis {
                                    Axis::TopBottom => p1.y > p2.y,
                                    Axis::LeftRight => p1.x > p2.x,
                                },
                            ));
                            drawing.changed_in_ui();
                            tools.clear();
                            return;
                        }
                        _ => {}
                    }
                }

                drawing.add_constraint(Constraint::LineAlongCardinal(
                    ConstraintMeta::default(),
                    k,
                    want_axis,
                ));
                tools.clear();
            }
            ToolResponse::NewPointLerp(p_fk, l_fk) => {
                match (drawing.features.get(p_fk), drawing.features.get(l_fk)) {
                    (Some(Feature::Point(..)), Some(Feature::LineSegment(..))) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::PointLerpLine(
                            ConstraintMeta::default(),
                            l_fk,
                            p_fk,
                            0.5,
                        ));

                        tools.clear();
                    }
                    _ => {}
                }
            }
            ToolResponse::NewEqual(l1, l2) => {
                match (drawing.features.get(l1), drawing.features.get(l2)) {
                    (Some(Feature::LineSegment(..)), Some(Feature::LineSegment(..))) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::LineLengthsEqual(
                            ConstraintMeta::default(),
                            l1,
                            l2,
                            None,
                        ));

                        tools.clear();
                    }
                    (Some(Feature::Circle(..)), Some(Feature::Circle(..))) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::CircleRadiusEqual(
                            ConstraintMeta::default(),
                            l1,
                            l2,
                            None,
                        ));

                        tools.clear();
                    }
                    _ => {}
                }
            }
            ToolResponse::ConstraintLinesEqualRemoveMultiplier(ck) => {
                match drawing.constraints.get_mut(ck) {
                    Some(Constraint::LineLengthsEqual(_meta, _l1, _l2, multiplier)) => {
                        *multiplier = None;
                        drawing.changed_in_ui();
                    }
                    _ => {
                        unreachable!();
                    }
                }
            }

            ToolResponse::NewCircleRadiusConstraint(k) => match drawing.features.get(k) {
                Some(Feature::Circle(_, _, radius)) => {
                    drawing.add_constraint(Constraint::CircleRadius(
                        ConstraintMeta::default(),
                        k,
                        *radius,
                        DimensionDisplay {
                            x: 35.0,
                            y: 35.0,
                            ..DimensionDisplay::default()
                        },
                    ));
                    tools.clear();
                }
                _ => {}
            },
            ToolResponse::ConstraintRadiusEqualRemoveMultiplier(ck) => {
                match drawing.constraints.get_mut(ck) {
                    Some(Constraint::CircleRadiusEqual(_meta, _l1, _l2, multiplier)) => {
                        *multiplier = None;
                        drawing.changed_in_ui();
                    }
                    _ => {
                        unreachable!();
                    }
                }
            }

            ToolResponse::NewParallelLine(l1, l2) => {
                match (drawing.features.get(l1), drawing.features.get(l2)) {
                    (Some(Feature::LineSegment(..)), Some(Feature::LineSegment(..))) => {
                        // TODO: Delete/modify existing constraints that would clash, if any

                        drawing.add_constraint(Constraint::LinesParallel(
                            ConstraintMeta::default(),
                            l1,
                            l2,
                        ));

                        tools.clear();
                    }
                    _ => {}
                }
            }

            ToolResponse::NewGlobalAngleConstraint(k) => match drawing.features.get(k) {
                Some(Feature::LineSegment(..)) => {
                    drawing.add_constraint(Constraint::LineAngle(
                        ConstraintMeta::default(),
                        k,
                        0.0,
                    ));
                    tools.clear();
                }
                _ => {}
            },

            ToolResponse::ArrayWizard(k, pos, info) => {
                let mut last_point = k;
                for n in 0..info.array_wizard_count {
                    let new_k = drawing.features.insert(Feature::Point(
                        FeatureMeta::default_construction(),
                        pos.x
                            + info
                                .array_wizard_direction
                                .extend((n + 1) as f32 * info.array_wizard_separation)
                                .x,
                        pos.y
                            + info
                                .array_wizard_direction
                                .extend((n + 1) as f32 * info.array_wizard_separation)
                                .y,
                    ));
                    let line = drawing.features.insert(Feature::LineSegment(
                        FeatureMeta::default_construction(),
                        last_point,
                        new_k,
                    ));

                    use crate::data::Direction::*;
                    drawing.add_constraint(Constraint::LineLength(
                        ConstraintMeta::default(),
                        line,
                        info.array_wizard_separation,
                        match info.array_wizard_direction {
                            Up => Some((Axis::TopBottom, true)),
                            Down => Some((Axis::TopBottom, false)),
                            Left => Some((Axis::LeftRight, true)),
                            Right => Some((Axis::LeftRight, false)),
                        },
                        DimensionDisplay {
                            x: 0.,
                            y: 35.0,
                            ..DimensionDisplay::default()
                        },
                    ));
                    last_point = new_k;
                }
            }
        }
    }
}
