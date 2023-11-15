use crate::l::LineSegment;
use crate::system::{self, TermAllocator, TermRef, TermType};
use crate::{Feature, FeatureKey};
use slotmap::HopSlotMap;

slotmap::new_key_type! {
    pub struct ConstraintKey;
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct ConstraintMeta {}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum Constraint {
    Fixed(ConstraintMeta, FeatureKey, f32, f32),
    LineLength(ConstraintMeta, FeatureKey, f32, (f32, f32)),
}

impl Constraint {
    pub fn affecting_features(&self) -> Vec<FeatureKey> {
        use Constraint::{Fixed, LineLength};
        match self {
            Fixed(_, fk, _, _) => vec![fk.clone()],
            LineLength(_, fk, ..) => vec![fk.clone()],
        }
    }

    pub fn valid_for_feature(&self, ft: &Feature) -> bool {
        use Constraint::{Fixed, LineLength};
        match self {
            Fixed(..) => matches!(ft, &Feature::Point(..)),
            LineLength(..) => matches!(ft, &Feature::LineSegment(..)),
        }
    }

    pub fn conflicts(&self, other: &Constraint) -> bool {
        use Constraint::{Fixed, LineLength};
        match (self, other) {
            (Fixed(_, f1, _, _), Fixed(_, f2, _, _)) => f1 == f2,
            (LineLength(_, f1, ..), LineLength(_, f2, ..)) => f1 == f2,
            _ => false,
        }
    }

    pub fn screen_dist_sq(
        &self,
        drawing: &crate::Data,
        hp: egui::Pos2,
        vp: &crate::Viewport,
    ) -> Option<f32> {
        use Constraint::{Fixed, LineLength};
        match self {
            Fixed(..) => None,
            LineLength(_, fk, _, (ref_x, ref_y)) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = drawing.features.get(*fk) {
                    let (a, b) = match (
                        drawing.features.get(*f1).unwrap(),
                        drawing.features.get(*f2).unwrap(),
                    ) {
                        (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                            (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                        }
                        _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                    };

                    let reference = egui::Vec2::new(*ref_x, *ref_y);
                    let t = (a - b).angle() + reference.angle();
                    let text_center = vp.translate_point(a.lerp(b, 0.5))
                        + egui::Vec2::angled(t) * reference.length();

                    let bounds = egui::Rect::from_center_size(text_center, (60., 15.).into());
                    Some(bounds.distance_sq_to_pos(hp))
                } else {
                    None
                }
            }
        }
    }

    pub fn paint(
        &self,
        drawing: &crate::Data,
        _k: ConstraintKey,
        params: &crate::PaintParams,
        painter: &egui::Painter,
    ) {
        use Constraint::{Fixed, LineLength};
        match self {
            Fixed(_, k, _, _) => {
                if let Some(Feature::Point(_, x, y)) = drawing.features.get(*k) {
                    let c = params.vp.translate_point(egui::Pos2 { x: *x, y: *y });
                    painter.circle_stroke(
                        c,
                        7.,
                        egui::Stroke {
                            width: 1.,
                            color: params.colors.text,
                        },
                    );
                };
            }

            LineLength(_, k, d, (ref_x, ref_y)) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = drawing.features.get(*k) {
                    let (a, b) = match (
                        drawing.features.get(*f1).unwrap(),
                        drawing.features.get(*f2).unwrap(),
                    ) {
                        (Feature::Point(_, x1, y1), Feature::Point(_, x2, y2)) => {
                            (egui::Pos2 { x: *x1, y: *y1 }, egui::Pos2 { x: *x2, y: *y2 })
                        }
                        _ => panic!("unexpected subkey types: {:?} & {:?}", f1, f2),
                    };

                    crate::l::draw::DimensionLengthOverlay {
                        a,
                        b,
                        val: d,
                        reference: egui::Vec2::new(*ref_x, *ref_y),
                        hovered: params.hovered,
                        selected: params.selected,
                    }
                    .draw(painter, params);
                }
            }
        }
    }
}

impl system::ConstraintProvider<ConstraintResidualIter> for Constraint {
    fn residuals(
        &self,
        features: &mut HopSlotMap<FeatureKey, Feature>,
        allocator: &mut TermAllocator,
    ) -> ConstraintResidualIter {
        use Constraint::{Fixed, LineLength};
        match self {
            Fixed(_, k, x, y) => ConstraintResidualIter::Fixed {
                count: 0,
                x_val: *x,
                y_val: *y,
                x_ref: allocator.get_feature_term(*k, TermType::PositionX),
                y_ref: allocator.get_feature_term(*k, TermType::PositionY),
            },
            LineLength(_, k, d, _) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = features.get(*k) {
                    ConstraintResidualIter::PointDistance {
                        count: 0,
                        distance: *d,
                        x1_ref: allocator.get_feature_term(*f1, TermType::PositionX),
                        y1_ref: allocator.get_feature_term(*f1, TermType::PositionY),
                        x2_ref: allocator.get_feature_term(*f2, TermType::PositionX),
                        y2_ref: allocator.get_feature_term(*f2, TermType::PositionY),
                    }
                } else {
                    panic!();
                }
            }
            _ => todo!(),
        }
    }
}

/// A non-allocating iterator over the residuals provided by a constraint.
pub(crate) enum ConstraintResidualIter {
    Fixed {
        count: usize,
        x_ref: TermRef,
        y_ref: TermRef,
        x_val: f32,
        y_val: f32,
    },
    PointDistance {
        count: usize,
        distance: f32,
        x1_ref: TermRef,
        y1_ref: TermRef,
        x2_ref: TermRef,
        y2_ref: TermRef,
    },
}

impl Iterator for ConstraintResidualIter {
    type Item = system::ResidualConstraint;

    // next() is the only required method
    fn next(&mut self) -> Option<Self::Item> {
        use eq::{Expression, Rational};
        match self {
            ConstraintResidualIter::Fixed {
                x_ref,
                y_ref,
                x_val,
                y_val,
                count,
            } => match count {
                0 => {
                    *count += 1;

                    Some(system::ResidualConstraint::new(
                        x_ref.clone(),
                        Expression::Rational(Rational::from_float(*x_val).unwrap(), true),
                    ))
                    // Expression::Difference(
                    //     Box::new(Expression::Variable("x".into())),
                    //     Box::new(Expression::Rational(
                    //         Rational::from_float(*x_val).unwrap(),
                    //         true,
                    //     )),
                    // ),
                }
                1 => {
                    *count += 1;

                    Some(system::ResidualConstraint::new(
                        y_ref.clone(),
                        Expression::Rational(Rational::from_float(*y_val).unwrap(), true),
                    ))
                }
                _ => None,
            },

            ConstraintResidualIter::PointDistance {
                x1_ref,
                y1_ref,
                x2_ref,
                y2_ref,
                distance,
                count,
            } => match count {
                0 => {
                    *count += 1;

                    Some(system::ResidualConstraint::new(
                        x2_ref.clone(),
                        Expression::Difference(
                            Box::new(Expression::Variable((&x1_ref.clone()).into())),
                            Box::new(Expression::Sqrt(
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Rational(
                                            Rational::from_float(*distance).unwrap(),
                                            true,
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Difference(
                                            Box::new(Expression::Variable(
                                                (&y2_ref.clone()).into(),
                                            )),
                                            Box::new(Expression::Variable(
                                                (&y1_ref.clone()).into(),
                                            )),
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                )),
                                true,
                            )),
                        ),
                    ))
                }
                1 => {
                    *count += 1;

                    Some(system::ResidualConstraint::new(
                        x1_ref.clone(),
                        Expression::Difference(
                            Box::new(Expression::Variable((&x2_ref.clone()).into())),
                            Box::new(Expression::Sqrt(
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Rational(
                                            Rational::from_float(*distance).unwrap(),
                                            true,
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Difference(
                                            Box::new(Expression::Variable(
                                                (&y2_ref.clone()).into(),
                                            )),
                                            Box::new(Expression::Variable(
                                                (&y1_ref.clone()).into(),
                                            )),
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                )),
                                true,
                            )),
                        ),
                    ))
                }
                2 => {
                    *count += 1;

                    Some(system::ResidualConstraint::new(
                        y2_ref.clone(),
                        Expression::Difference(
                            Box::new(Expression::Variable((&y1_ref.clone()).into())),
                            Box::new(Expression::Sqrt(
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Rational(
                                            Rational::from_float(*distance).unwrap(),
                                            true,
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Difference(
                                            Box::new(Expression::Variable(
                                                (&x2_ref.clone()).into(),
                                            )),
                                            Box::new(Expression::Variable(
                                                (&x1_ref.clone()).into(),
                                            )),
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                )),
                                true,
                            )),
                        ),
                    ))
                }
                3 => {
                    *count += 1;

                    Some(system::ResidualConstraint::new(
                        y1_ref.clone(),
                        Expression::Difference(
                            Box::new(Expression::Variable((&y2_ref.clone()).into())),
                            Box::new(Expression::Sqrt(
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Rational(
                                            Rational::from_float(*distance).unwrap(),
                                            true,
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                    Box::new(Expression::Power(
                                        Box::new(Expression::Difference(
                                            Box::new(Expression::Variable(
                                                (&x2_ref.clone()).into(),
                                            )),
                                            Box::new(Expression::Variable(
                                                (&x1_ref.clone()).into(),
                                            )),
                                        )),
                                        Box::new(Expression::Integer(2.into())),
                                    )),
                                )),
                                true,
                            )),
                        ),
                    ))
                }
                _ => None,
            },
        }
    }
}

impl ExactSizeIterator for ConstraintResidualIter {
    fn len(&self) -> usize {
        match self {
            ConstraintResidualIter::Fixed { count, .. } => 2 - count,
            ConstraintResidualIter::PointDistance { count, .. } => 4 - count,
        }
    }
}

impl core::iter::FusedIterator for ConstraintResidualIter {}
