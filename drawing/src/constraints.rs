use crate::system::{TermRef, TermType};
use crate::{Feature, FeatureKey};
use eq::{Expression, Rational};
use std::collections::HashMap;

slotmap::new_key_type! {
    pub struct ConstraintKey;
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct ConstraintMeta {
    #[serde(skip)]
    pub focus_to: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct DimensionDisplay {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum Axis {
    #[default]
    LeftRight,
    TopBottom,
}

impl Axis {
    pub fn swap(&mut self) {
        *self = match self {
            Axis::TopBottom => Axis::LeftRight,
            Axis::LeftRight => Axis::TopBottom,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SerializedConstraint {
    pub kind: String,
    pub meta: ConstraintMeta,
    pub feature_idx: Vec<usize>,

    /// Only used for Constraint::Fixed
    pub at: (f32, f32),
    /// Only used for Constraint::LineLength & Constraint::PointLerpLine
    pub amt: f32,
    /// Only used for Constraint::LineLength
    pub cardinality: Option<(Axis, bool)>,
    /// Only used for Constraint::LineLength
    pub ref_offset: DimensionDisplay,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    Fixed(ConstraintMeta, FeatureKey, f32, f32),
    LineLength(
        ConstraintMeta,
        FeatureKey,
        f32,
        Option<(Axis, bool)>, // true = negative relationship
        DimensionDisplay,
    ),
    LineAlongCardinal(ConstraintMeta, FeatureKey, Axis),
    PointLerpLine(ConstraintMeta, FeatureKey, FeatureKey, f32),
    LineLengthsEqual(ConstraintMeta, FeatureKey, FeatureKey, Option<f32>),
    LinesParallel(ConstraintMeta, FeatureKey, FeatureKey),
    LineAngle(ConstraintMeta, FeatureKey, f32),

    CircleRadius(ConstraintMeta, FeatureKey, f32, DimensionDisplay),
}

impl Constraint {
    pub fn affecting_features(&self) -> Vec<FeatureKey> {
        use Constraint::{
            CircleRadius, Fixed, LineAlongCardinal, LineAngle, LineLength, LineLengthsEqual,
            LinesParallel, PointLerpLine,
        };
        match self {
            Fixed(_, fk, ..) => vec![fk.clone()],
            LineLength(_, fk, ..) => vec![fk.clone()],
            LineAlongCardinal(_, fk, ..) => vec![fk.clone()],
            PointLerpLine(_, l_fk, p_fk, _) => vec![l_fk.clone(), p_fk.clone()],
            LineLengthsEqual(_, l1, l2, ..) => vec![l1.clone(), l2.clone()],
            LinesParallel(_, l1, l2, ..) => vec![l1.clone(), l2.clone()],
            LineAngle(_, fk, ..) => vec![fk.clone()],
            CircleRadius(_, fk, ..) => vec![fk.clone()],
        }
    }

    pub fn valid_for_feature(&self, ft: &Feature) -> bool {
        use Constraint::{
            CircleRadius, Fixed, LineAlongCardinal, LineAngle, LineLength, LineLengthsEqual,
            LinesParallel, PointLerpLine,
        };
        match self {
            Fixed(..) => matches!(ft, &Feature::Point(..)),
            LineLength(..) => matches!(ft, &Feature::LineSegment(..)),
            LineAlongCardinal(..) => matches!(ft, &Feature::LineSegment(..)),
            PointLerpLine(..) => matches!(ft, &Feature::LineSegment(..)),
            LineLengthsEqual(..) => matches!(ft, &Feature::LineSegment(..)),
            LinesParallel(..) => matches!(ft, &Feature::LineSegment(..)),
            LineAngle(..) => matches!(ft, &Feature::LineSegment(..)),
            CircleRadius(..) => matches!(ft, &Feature::Circle(..)),
        }
    }

    pub fn conflicts(&self, other: &Constraint) -> bool {
        use Constraint::{
            CircleRadius, Fixed, LineAlongCardinal, LineAngle, LineLength, LineLengthsEqual,
            LinesParallel, PointLerpLine,
        };
        match (self, other) {
            (Fixed(_, f1, _, _), Fixed(_, f2, _, _)) => f1 == f2,
            (LineLength(_, f1, ..), LineLength(_, f2, ..)) => f1 == f2,
            (LineLength(_, f1, _d, Some(_axis), ..), LineAlongCardinal(_, f2, ..)) => f1 == f2,
            (LineAlongCardinal(_, f2, ..), LineLength(_, f1, _d, Some(_axis), ..)) => f1 == f2,
            (LineAlongCardinal(_, f1, ..), LineAlongCardinal(_, f2, ..)) => f1 == f2,
            (PointLerpLine(_, l_fk1, p_fk1, _), PointLerpLine(_, l_fk2, p_fk2, _)) => {
                l_fk1 == l_fk2 && p_fk1 == p_fk2
            }
            (LineLengthsEqual(_, l11, l12, ..), LineLengthsEqual(_, l21, l22, ..)) => {
                (l11 == l21 && l12 == l22) || (l11 == l22 && l12 == l21)
            }
            (LinesParallel(_, l11, l12, ..), LinesParallel(_, l21, l22, ..)) => {
                (l11 == l21 && l12 == l22) || (l11 == l22 && l12 == l21)
            }
            (LineAngle(_, f1, ..), LineAngle(_, f2, ..)) => f1 == f2,
            (CircleRadius(_, f1, ..), CircleRadius(_, f2, ..)) => f1 == f2,
            _ => false,
        }
    }

    pub fn screen_dist_sq(
        &self,
        drawing: &crate::Data,
        hp: egui::Pos2,
        vp: &crate::Viewport,
    ) -> Option<f32> {
        use Constraint::{
            CircleRadius, Fixed, LineAlongCardinal, LineAngle, LineLength, LineLengthsEqual,
            LinesParallel, PointLerpLine,
        };
        match self {
            Fixed(..) => None,
            LineLength(_, fk, _, _, dd) => {
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

                    let reference = egui::Vec2::new(dd.x, dd.y);
                    let t = (a - b).angle() + reference.angle();
                    let text_center = vp.translate_point(a.lerp(b, 0.5))
                        + egui::Vec2::angled(t) * reference.length();

                    let bounds = egui::Rect::from_center_size(text_center, (60., 15.).into());
                    Some(bounds.distance_sq_to_pos(hp))
                } else {
                    unreachable!();
                }
            }
            LineAlongCardinal(_, fk, ..) => {
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

                    let text_center = vp.translate_point(a.lerp(b, 0.5));
                    let bounds = egui::Rect::from_center_size(text_center, (20., 15.).into());
                    Some(bounds.distance_sq_to_pos(hp))
                } else {
                    unreachable!();
                }
            }
            CircleRadius(_, fk, _, dd) => {
                if let Some(Feature::Circle(_, f1, _r)) = drawing.features.get(*fk) {
                    let center = match drawing.features.get(*f1).unwrap() {
                        Feature::Point(_, x1, y1) => egui::Pos2 { x: *x1, y: *y1 },
                        _ => panic!("unexpected subkey type: {:?}", f1),
                    };

                    let reference = egui::Vec2::new(dd.x, dd.y);
                    let text_center = vp.translate_point(center) + reference;
                    let bounds = egui::Rect::from_center_size(text_center, (60., 15.).into());
                    Some(bounds.distance_sq_to_pos(hp))
                } else {
                    unreachable!();
                }
            }
            PointLerpLine(..) => None,
            LineLengthsEqual(..) => None,
            LinesParallel(..) => None,
            LineAngle(..) => None,
        }
    }

    pub fn paint(
        &self,
        drawing: &crate::Data,
        _k: ConstraintKey,
        params: &crate::PaintParams,
        painter: &egui::Painter,
    ) {
        use Constraint::{
            CircleRadius, Fixed, LineAlongCardinal, LineAngle, LineLength, LineLengthsEqual,
            LinesParallel, PointLerpLine,
        };
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

            LineLength(_, k, d, aa_info, dd) => {
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
                        val: &match aa_info {
                            None => format!("{:.3}", d),
                            Some((Axis::LeftRight, false)) => format!("H+{:.3}", d),
                            Some((Axis::LeftRight, true)) => format!("H-{:.3}", d),
                            Some((Axis::TopBottom, false)) => format!("V+{:.3}", d),
                            Some((Axis::TopBottom, true)) => format!("V+{:.3}", d),
                        },
                        reference: egui::Vec2::new(dd.x, dd.y),
                        hovered: params.hovered,
                        selected: params.selected,
                    }
                    .draw(painter, params);
                }
            }

            LineAlongCardinal(_, k, axis) => {
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

                    let mid = params.vp.translate_point(a.lerp(b, 0.5));
                    painter.text(
                        mid,
                        egui::Align2::CENTER_CENTER,
                        if *axis == Axis::LeftRight { "H" } else { "V" },
                        params.font_id.clone(),
                        egui::Color32::WHITE,
                    );
                }
            }

            PointLerpLine(..) => {}
            LineLengthsEqual(..) => {}
            LinesParallel(..) => {}
            LineAngle(..) => {}

            CircleRadius(_meta, fk, radius, dd) => {
                if let Some(Feature::Circle(_, center_fk, ..)) = drawing.features.get(*fk) {
                    let center = match drawing.features.get(*center_fk).unwrap() {
                        Feature::Point(_, x1, y1) => egui::Pos2 { x: *x1, y: *y1 },
                        _ => panic!("unexpected subkey type: {:?}", center_fk),
                    };

                    crate::l::draw::DimensionRadiusOverlay {
                        center: center,
                        radius: radius,
                        val: &format!("R {:.3}", radius),
                        reference: egui::Vec2::new(dd.x, dd.y),
                        hovered: params.hovered,
                        selected: params.selected,
                    }
                    .draw(painter, params);
                }
            }
        }
    }

    pub fn equations(&self, drawing: &mut crate::Data) -> Vec<Expression> {
        use Constraint::{
            CircleRadius, Fixed, LineAlongCardinal, LineAngle, LineLength, LineLengthsEqual,
            LinesParallel, PointLerpLine,
        };
        match self {
            Fixed(_, k, x, y) => {
                let (tx, ty) = (
                    &drawing.terms.get_feature_term(*k, TermType::PositionX),
                    &drawing.terms.get_feature_term(*k, TermType::PositionY),
                );
                vec![
                    Expression::Equal(
                        Box::new(Expression::Variable(tx.into())),
                        Box::new(Expression::Rational(
                            Rational::from_float(*x).unwrap(),
                            true,
                        )),
                    ),
                    Expression::Equal(
                        Box::new(Expression::Variable(ty.into())),
                        Box::new(Expression::Rational(
                            Rational::from_float(*y).unwrap(),
                            true,
                        )),
                    ),
                ]
            }
            CircleRadius(_, k, r, _) => {
                let cr = &drawing.terms.get_feature_term(*k, TermType::ScalarRadius);
                vec![Expression::Equal(
                    Box::new(Expression::Variable(cr.into())),
                    Box::new(Expression::Rational(
                        Rational::from_float(*r).unwrap(),
                        true,
                    )),
                )]
            }
            LineLength(_, k, d, aa_info, _) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = drawing.features.get(*k) {
                    let td = &drawing.terms.get_feature_term(*k, TermType::ScalarDistance);
                    let (x1, y1, x2, y2) = (
                        &drawing.terms.get_feature_term(*f1, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f1, TermType::PositionY),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionY),
                    );

                    match aa_info {
                        Some((Axis::LeftRight, is_neg)) => vec![
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(Expression::Rational(
                                    Rational::from_float(*d).unwrap(),
                                    true,
                                )),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(if *is_neg {
                                    Expression::Difference(
                                        Box::new(Expression::Variable(x1.into())),
                                        Box::new(Expression::Variable(x2.into())),
                                    )
                                } else {
                                    Expression::Difference(
                                        Box::new(Expression::Variable(x2.into())),
                                        Box::new(Expression::Variable(x1.into())),
                                    )
                                }),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(y2.into())),
                                Box::new(Expression::Variable(y1.into())),
                            ),
                        ],
                        Some((Axis::TopBottom, is_neg)) => vec![
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(Expression::Rational(
                                    Rational::from_float(*d).unwrap(),
                                    true,
                                )),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(if *is_neg {
                                    Expression::Difference(
                                        Box::new(Expression::Variable(y1.into())),
                                        Box::new(Expression::Variable(y2.into())),
                                    )
                                } else {
                                    Expression::Difference(
                                        Box::new(Expression::Variable(y2.into())),
                                        Box::new(Expression::Variable(y1.into())),
                                    )
                                }),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(x2.into())),
                                Box::new(Expression::Variable(x1.into())),
                            ),
                        ],
                        None => vec![
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(Expression::Rational(
                                    Rational::from_float(*d).unwrap(),
                                    true,
                                )),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(distance_eq(td, x1, y1, x2, y2)),
                            ),
                        ],
                    }
                } else {
                    unreachable!();
                }
            }

            LineAlongCardinal(_, k, axis) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = drawing.features.get(*k) {
                    let (x1, y1, x2, y2) = (
                        &drawing.terms.get_feature_term(*f1, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f1, TermType::PositionY),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionY),
                    );

                    if *axis == Axis::LeftRight {
                        vec![Expression::Equal(
                            Box::new(Expression::Variable(y1.into())),
                            Box::new(Expression::Variable(y2.into())),
                        )]
                    } else {
                        vec![Expression::Equal(
                            Box::new(Expression::Variable(x1.into())),
                            Box::new(Expression::Variable(x2.into())),
                        )]
                    }
                } else {
                    unreachable!();
                }
            }

            PointLerpLine(_, l_fk, p_fk, amt) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = drawing.features.get(*l_fk) {
                    let (x1, y1, x2, y2, x3, y3) = (
                        &drawing.terms.get_feature_term(*f1, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f1, TermType::PositionY),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionY),
                        &drawing.terms.get_feature_term(*p_fk, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p_fk, TermType::PositionY),
                    );

                    vec![
                        Expression::Equal(
                            Box::new(Expression::Variable(x3.into())),
                            Box::new(Expression::Sum(
                                Box::new(Expression::Variable(x1.into())),
                                Box::new(Expression::Product(
                                    Box::new(Expression::Rational(
                                        Rational::from_float(*amt).unwrap(),
                                        true,
                                    )),
                                    Box::new(Expression::Difference(
                                        Box::new(Expression::Variable(x2.into())),
                                        Box::new(Expression::Variable(x1.into())),
                                    )),
                                )),
                            )),
                        ),
                        Expression::Equal(
                            Box::new(Expression::Variable(y3.into())),
                            Box::new(Expression::Sum(
                                Box::new(Expression::Variable(y1.into())),
                                Box::new(Expression::Product(
                                    Box::new(Expression::Rational(
                                        Rational::from_float(*amt).unwrap(),
                                        true,
                                    )),
                                    Box::new(Expression::Difference(
                                        Box::new(Expression::Variable(y2.into())),
                                        Box::new(Expression::Variable(y1.into())),
                                    )),
                                )),
                            )),
                        ),
                    ]
                } else {
                    unreachable!();
                }
            }

            LineLengthsEqual(_, l1, l2, multiplier, ..) => {
                if let (
                    Some(Feature::LineSegment(_, p11, p12)),
                    Some(Feature::LineSegment(_, p21, p22)),
                ) = (drawing.features.get(*l1), drawing.features.get(*l2))
                {
                    let d1 = &drawing
                        .terms
                        .get_feature_term(*l1, TermType::ScalarDistance);
                    let d2 = &drawing
                        .terms
                        .get_feature_term(*l2, TermType::ScalarDistance);

                    let (x11, y11, x12, y12) = (
                        &drawing.terms.get_feature_term(*p11, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p11, TermType::PositionY),
                        &drawing.terms.get_feature_term(*p12, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p12, TermType::PositionY),
                    );
                    let (x21, y21, x22, y22) = (
                        &drawing.terms.get_feature_term(*p21, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p21, TermType::PositionY),
                        &drawing.terms.get_feature_term(*p22, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p22, TermType::PositionY),
                    );

                    vec![
                        Expression::Equal(
                            Box::new(Expression::Variable(d2.into())),
                            Box::new(match multiplier {
                                Some(a) => Expression::Product(
                                    Box::new(Expression::Rational(
                                        Rational::from_float(*a).unwrap(),
                                        true,
                                    )),
                                    Box::new(Expression::Variable(d1.into())),
                                ),
                                None => Expression::Variable(d1.into()),
                            }),
                        ),
                        Expression::Equal(
                            Box::new(Expression::Variable(d1.into())),
                            Box::new(distance_eq(d1, x11, y11, x12, y12)),
                        ),
                        Expression::Equal(
                            Box::new(Expression::Variable(d2.into())),
                            Box::new(distance_eq(d2, x21, y21, x22, y22)),
                        ),
                    ]
                } else {
                    unreachable!();
                }
            }

            LineAngle(_, l1, angle, ..) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = drawing.features.get(*l1) {
                    let td = &drawing
                        .terms
                        .get_feature_term(*l1, TermType::ScalarDistance);
                    let ta = &drawing
                        .terms
                        .get_feature_term(*l1, TermType::ScalarGlobalAngle);
                    let (x1, y1, x2, y2) = (
                        &drawing.terms.get_feature_term(*f1, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f1, TermType::PositionY),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionY),
                    );

                    vec![
                        Expression::Equal(
                            Box::new(Expression::Variable(ta.into())),
                            Box::new(Expression::Rational(
                                Rational::from_float(angle.cos()).unwrap(),
                                true,
                            )),
                        ),
                        Expression::Equal(
                            Box::new(Expression::Variable(ta.into())),
                            Box::new(Expression::Neg(Box::new(cosine_angle_eq(
                                td, x1, y1, x2, y2,
                            )))),
                        ),
                        // Expression::Equal(
                        //     Box::new(Expression::Variable(y2.into())),
                        //     Box::new(Expression::Sum(
                        //         Box::new(Expression::Variable(y1.into())),
                        //         Box::new(Expression::Product(
                        //             Box::new(distance_eq(td, x1, y1, x2, y2)),
                        //             Box::new(Expression::Trig(
                        //                 TrigOp::Sin,
                        //                 Box::new(Expression::Variable(ta.into())),
                        //             )),
                        //         )),
                        //     )),
                        // ),
                    ]
                } else {
                    unreachable!();
                }
            }

            LinesParallel(_, l1, l2, ..) => {
                if let (
                    Some(Feature::LineSegment(_, p11, p12)),
                    Some(Feature::LineSegment(_, p21, p22)),
                ) = (drawing.features.get(*l1), drawing.features.get(*l2))
                {
                    let (x11, y11, x12, y12) = (
                        &drawing.terms.get_feature_term(*p11, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p11, TermType::PositionY),
                        &drawing.terms.get_feature_term(*p12, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p12, TermType::PositionY),
                    );
                    let (x21, y21, x22, y22) = (
                        &drawing.terms.get_feature_term(*p21, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p21, TermType::PositionY),
                        &drawing.terms.get_feature_term(*p22, TermType::PositionX),
                        &drawing.terms.get_feature_term(*p22, TermType::PositionY),
                    );

                    vec![Expression::Equal(
                        Box::new(Expression::Integer(0.into())),
                        Box::new(Expression::Difference(
                            Box::new(Expression::Product(
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Variable(x12.into())),
                                    Box::new(Expression::Variable(x11.into())),
                                )),
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Variable(y22.into())),
                                    Box::new(Expression::Variable(y21.into())),
                                )),
                            )),
                            Box::new(Expression::Product(
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Variable(y12.into())),
                                    Box::new(Expression::Variable(y11.into())),
                                )),
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Variable(x22.into())),
                                    Box::new(Expression::Variable(x21.into())),
                                )),
                            )),
                        )),
                    )]
                } else {
                    unreachable!();
                }
            }
        }
    }

    /// Serialize returns a structure suitable for serialization to disk. Any feature
    /// which maybe referenced from the current constraint must be present in fk_to_idx.
    pub fn serialize(
        &self,
        fk_to_idx: &HashMap<FeatureKey, usize>,
    ) -> Result<SerializedConstraint, ()> {
        match self {
            Constraint::Fixed(meta, fk, x, y) => Ok(SerializedConstraint {
                kind: "fixed".to_string(),
                meta: meta.clone(),
                feature_idx: vec![*fk_to_idx.get(fk).ok_or(())?],
                at: (*x, *y),
                ..SerializedConstraint::default()
            }),
            Constraint::LineLength(meta, fk, d, axis, ref_offset) => Ok(SerializedConstraint {
                kind: "length".to_string(),
                meta: meta.clone(),
                feature_idx: vec![*fk_to_idx.get(fk).ok_or(())?],
                amt: *d,
                cardinality: axis.clone(),
                ref_offset: ref_offset.clone(),
                ..SerializedConstraint::default()
            }),
            Constraint::LineAngle(meta, fk, amt) => Ok(SerializedConstraint {
                kind: "line_angle".to_string(),
                meta: meta.clone(),
                feature_idx: vec![*fk_to_idx.get(fk).ok_or(())?],
                amt: *amt,
                ..SerializedConstraint::default()
            }),

            Constraint::LineAlongCardinal(meta, fk, Axis::TopBottom) => Ok(SerializedConstraint {
                kind: "vertical".to_string(),
                meta: meta.clone(),
                feature_idx: vec![*fk_to_idx.get(fk).ok_or(())?],
                ..SerializedConstraint::default()
            }),
            Constraint::LineAlongCardinal(meta, fk, Axis::LeftRight) => Ok(SerializedConstraint {
                kind: "horizontal".to_string(),
                meta: meta.clone(),
                feature_idx: vec![*fk_to_idx.get(fk).ok_or(())?],
                ..SerializedConstraint::default()
            }),

            Constraint::PointLerpLine(meta, fk1, fk2, amt) => {
                let (fk1_idx, fk2_idx) =
                    (fk_to_idx.get(fk1).ok_or(())?, fk_to_idx.get(fk2).ok_or(())?);

                Ok(SerializedConstraint {
                    kind: "point_lerp".to_string(),
                    meta: meta.clone(),
                    feature_idx: vec![*fk1_idx, *fk2_idx],
                    amt: *amt,
                    ..SerializedConstraint::default()
                })
            }
            Constraint::LineLengthsEqual(meta, fk1, fk2, ratio) => {
                let (fk1_idx, fk2_idx) =
                    (fk_to_idx.get(fk1).ok_or(())?, fk_to_idx.get(fk2).ok_or(())?);

                Ok(SerializedConstraint {
                    kind: "line_lengths_equal".to_string(),
                    meta: meta.clone(),
                    feature_idx: vec![*fk1_idx, *fk2_idx],
                    amt: ratio.unwrap_or(0.0),
                    ..SerializedConstraint::default()
                })
            }

            Constraint::LinesParallel(meta, fk1, fk2) => {
                let (fk1_idx, fk2_idx) =
                    (fk_to_idx.get(fk1).ok_or(())?, fk_to_idx.get(fk2).ok_or(())?);

                Ok(SerializedConstraint {
                    kind: "lines_parallel".to_string(),
                    meta: meta.clone(),
                    feature_idx: vec![*fk1_idx, *fk2_idx],
                    ..SerializedConstraint::default()
                })
            }

            Constraint::CircleRadius(meta, fk, r, ref_offset) => Ok(SerializedConstraint {
                kind: "radius".to_string(),
                meta: meta.clone(),
                feature_idx: vec![*fk_to_idx.get(fk).ok_or(())?],
                amt: *r,
                ref_offset: ref_offset.clone(),
                ..SerializedConstraint::default()
            }),
        }
    }

    pub fn deserialize(
        sc: SerializedConstraint,
        idx_to_fk: &HashMap<usize, FeatureKey>,
    ) -> Result<Self, ()> {
        match sc.kind.as_str() {
            "fixed" => {
                if sc.feature_idx.len() < 1 {
                    return Err(());
                }
                Ok(Self::Fixed(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    sc.at.0,
                    sc.at.1,
                ))
            }
            "length" => {
                if sc.feature_idx.len() < 1 {
                    return Err(());
                }
                Ok(Self::LineLength(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    sc.amt,
                    sc.cardinality,
                    sc.ref_offset,
                ))
            }
            "line_angle" => {
                if sc.feature_idx.len() < 1 {
                    return Err(());
                }
                Ok(Self::LineAngle(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    sc.amt,
                ))
            }

            "vertical" => {
                if sc.feature_idx.len() < 1 {
                    return Err(());
                }
                Ok(Self::LineAlongCardinal(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    Axis::TopBottom,
                ))
            }
            "horizontal" => {
                if sc.feature_idx.len() < 1 {
                    return Err(());
                }
                Ok(Self::LineAlongCardinal(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    Axis::LeftRight,
                ))
            }

            "point_lerp" => {
                if sc.feature_idx.len() < 2 {
                    return Err(());
                }
                Ok(Self::PointLerpLine(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    *idx_to_fk.get(&sc.feature_idx[1]).ok_or(())?,
                    sc.amt,
                ))
            }
            "line_lengths_equal" => {
                if sc.feature_idx.len() < 2 {
                    return Err(());
                }
                Ok(Self::LineLengthsEqual(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    *idx_to_fk.get(&sc.feature_idx[1]).ok_or(())?,
                    if sc.amt == 0.0 { None } else { Some(sc.amt) },
                ))
            }
            "lines_parallel" => {
                if sc.feature_idx.len() < 2 {
                    return Err(());
                }
                Ok(Self::LinesParallel(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    *idx_to_fk.get(&sc.feature_idx[1]).ok_or(())?,
                ))
            }

            "radius" => {
                if sc.feature_idx.len() < 1 {
                    return Err(());
                }
                Ok(Self::CircleRadius(
                    sc.meta,
                    *idx_to_fk.get(&sc.feature_idx[0]).ok_or(())?,
                    sc.amt,
                    sc.ref_offset,
                ))
            }
            _ => Err(()),
        }
    }
}

fn distance_eq(_d: &TermRef, x1: &TermRef, y1: &TermRef, x2: &TermRef, y2: &TermRef) -> Expression {
    Expression::Sqrt(
        Box::new(Expression::Sum(
            Box::new(Expression::Power(
                Box::new(Expression::Difference(
                    Box::new(Expression::Variable(x2.into())),
                    Box::new(Expression::Variable(x1.into())),
                )),
                Box::new(Expression::Integer(2.into())),
            )),
            Box::new(Expression::Power(
                Box::new(Expression::Difference(
                    Box::new(Expression::Variable(y2.into())),
                    Box::new(Expression::Variable(y1.into())),
                )),
                Box::new(Expression::Integer(2.into())),
            )),
        )),
        true,
    )
}

fn cosine_angle_eq(
    d: &TermRef,
    x1: &TermRef,
    y1: &TermRef,
    x2: &TermRef,
    y2: &TermRef,
) -> Expression {
    // dot = ax × bx + ay × by
    // a = [1, -1]

    let dot = Expression::Sum(
        Box::new(Expression::Product(
            Box::new(Expression::Integer(1.into())),
            Box::new(Expression::Difference(
                Box::new(Expression::Variable(x2.into())),
                Box::new(Expression::Variable(x1.into())),
            )),
        )),
        Box::new(Expression::Product(
            Box::new(Expression::Integer((-1).into())),
            Box::new(Expression::Difference(
                Box::new(Expression::Variable(y2.into())),
                Box::new(Expression::Variable(y1.into())),
            )),
        )),
    );

    // The cosine of the angle between two vectors is equal to the dot product of the vectors,
    // divided by the product of their magnitude.
    // a magnitude is sqrt(1), so we just need to use b's magnitude which is just its distance.
    Expression::Quotient(
        Box::new(dot),
        Box::new(Expression::Product(
            Box::new(Expression::Variable(d.into())),
            Box::new(Expression::Sqrt(
                Box::new(Expression::Integer(1.into())),
                false,
            )),
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize() {
        use slotmap::Key;
        let point_key = FeatureKey::null();

        assert_eq!(
            Constraint::Fixed(ConstraintMeta::default(), point_key, 2.0, 1.0)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "fixed".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42],
                at: (2.0, 1.0),
                ..SerializedConstraint::default()
            }),
        );

        assert_eq!(
            Constraint::LineLength(
                ConstraintMeta::default(),
                point_key,
                85.0,
                None,
                DimensionDisplay { x: 2.0, y: 5.0 }
            )
            .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "length".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42],
                amt: 85.0,
                ref_offset: DimensionDisplay { x: 2.0, y: 5.0 },
                ..SerializedConstraint::default()
            }),
        );

        assert_eq!(
            Constraint::LineAlongCardinal(ConstraintMeta::default(), point_key, Axis::TopBottom,)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "vertical".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42],
                ..SerializedConstraint::default()
            }),
        );
        assert_eq!(
            Constraint::LineAlongCardinal(ConstraintMeta::default(), point_key, Axis::LeftRight,)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "horizontal".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42],
                ..SerializedConstraint::default()
            }),
        );

        assert_eq!(
            Constraint::PointLerpLine(ConstraintMeta::default(), point_key, point_key, 0.5,)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "point_lerp".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42, 42],
                amt: 0.5,
                ..SerializedConstraint::default()
            }),
        );

        assert_eq!(
            Constraint::LineLengthsEqual(ConstraintMeta::default(), point_key, point_key, None)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "line_lengths_equal".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42, 42],
                ..SerializedConstraint::default()
            }),
        );
        assert_eq!(
            Constraint::LineLengthsEqual(
                ConstraintMeta::default(),
                point_key,
                point_key,
                Some(0.5)
            )
            .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "line_lengths_equal".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42, 42],
                amt: 0.5,
                ..SerializedConstraint::default()
            }),
        );
        assert_eq!(
            Constraint::LinesParallel(ConstraintMeta::default(), point_key, point_key,)
                .serialize(&HashMap::from([(point_key, 42)])),
            Ok(SerializedConstraint {
                kind: "lines_parallel".to_string(),
                meta: ConstraintMeta::default(),
                feature_idx: vec![42, 42],
                ..SerializedConstraint::default()
            }),
        );
    }

    #[test]
    fn deserialize() {
        use slotmap::Key;
        let k = FeatureKey::null();

        assert_eq!(
            Constraint::deserialize(
                SerializedConstraint {
                    kind: "fixed".to_string(),
                    feature_idx: vec![1],
                    at: (2.0, 1.0),
                    ..SerializedConstraint::default()
                },
                &HashMap::from([(1, k)])
            )
            .unwrap(),
            Constraint::Fixed(ConstraintMeta::default(), k, 2.0, 1.0),
        );

        assert_eq!(
            Constraint::deserialize(
                SerializedConstraint {
                    kind: "vertical".to_string(),
                    feature_idx: vec![1],
                    ..SerializedConstraint::default()
                },
                &HashMap::from([(1, k)])
            )
            .unwrap(),
            Constraint::LineAlongCardinal(ConstraintMeta::default(), k, Axis::TopBottom,),
        );

        assert_eq!(
            Constraint::deserialize(
                SerializedConstraint {
                    kind: "length".to_string(),
                    feature_idx: vec![1],
                    amt: 66.0,
                    cardinality: Some((Axis::LeftRight, false)),
                    ..SerializedConstraint::default()
                },
                &HashMap::from([(1, k)])
            )
            .unwrap(),
            Constraint::LineLength(
                ConstraintMeta::default(),
                k,
                66.0,
                Some((Axis::LeftRight, false)),
                DimensionDisplay::default(),
            ),
        );

        assert_eq!(
            Constraint::deserialize(
                SerializedConstraint {
                    kind: "line_lengths_equal".to_string(),
                    feature_idx: vec![1, 1],
                    ..SerializedConstraint::default()
                },
                &HashMap::from([(1, k)])
            )
            .unwrap(),
            Constraint::LineLengthsEqual(ConstraintMeta::default(), k, k, None,),
        );
        assert_eq!(
            Constraint::deserialize(
                SerializedConstraint {
                    kind: "line_lengths_equal".to_string(),
                    feature_idx: vec![1, 1],
                    amt: 0.5,
                    ..SerializedConstraint::default()
                },
                &HashMap::from([(1, k)])
            )
            .unwrap(),
            Constraint::LineLengthsEqual(ConstraintMeta::default(), k, k, Some(0.5),),
        );

        // TODO: PointLerpLine, LinesParallel, CircleRadius
    }
}
