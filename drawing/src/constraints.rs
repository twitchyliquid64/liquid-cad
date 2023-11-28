use crate::system::{self, TermAllocator, TermRef, TermType};
use crate::{Feature, FeatureKey};
use eq::{Expression, Rational};
use slotmap::HopSlotMap;

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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum Constraint {
    Fixed(ConstraintMeta, FeatureKey, f32, f32),
    LineLength(
        ConstraintMeta,
        FeatureKey,
        f32,
        Option<Axis>,
        DimensionDisplay,
    ),
    LineAlongCardinal(ConstraintMeta, FeatureKey, Axis),
}

impl Constraint {
    pub fn affecting_features(&self) -> Vec<FeatureKey> {
        use Constraint::{Fixed, LineAlongCardinal, LineLength};
        match self {
            Fixed(_, fk, ..) => vec![fk.clone()],
            LineLength(_, fk, ..) => vec![fk.clone()],
            LineAlongCardinal(_, fk, ..) => vec![fk.clone()],
        }
    }

    pub fn valid_for_feature(&self, ft: &Feature) -> bool {
        use Constraint::{Fixed, LineAlongCardinal, LineLength};
        match self {
            Fixed(..) => matches!(ft, &Feature::Point(..)),
            LineLength(..) => matches!(ft, &Feature::LineSegment(..)),
            LineAlongCardinal(_, fk, ..) => matches!(ft, &Feature::LineSegment(..)),
        }
    }

    pub fn conflicts(&self, other: &Constraint) -> bool {
        use Constraint::{Fixed, LineAlongCardinal, LineLength};
        match (self, other) {
            (Fixed(_, f1, _, _), Fixed(_, f2, _, _)) => f1 == f2,
            (LineLength(_, f1, ..), LineLength(_, f2, ..)) => f1 == f2,
            (LineLength(_, f1, _d, Some(_axis), ..), LineAlongCardinal(_, f2, ..)) => f1 == f2,
            (LineAlongCardinal(_, f2, ..), LineLength(_, f1, _d, Some(_axis), ..)) => f1 == f2,
            (LineAlongCardinal(_, f1, ..), LineAlongCardinal(_, f2, ..)) => f1 == f2,
            _ => false,
        }
    }

    pub fn screen_dist_sq(
        &self,
        drawing: &crate::Data,
        hp: egui::Pos2,
        vp: &crate::Viewport,
    ) -> Option<f32> {
        use Constraint::{Fixed, LineAlongCardinal, LineLength};
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
        }
    }

    pub fn paint(
        &self,
        drawing: &crate::Data,
        _k: ConstraintKey,
        params: &crate::PaintParams,
        painter: &egui::Painter,
    ) {
        use Constraint::{Fixed, LineAlongCardinal, LineLength};
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

            LineLength(_, k, d, axis, dd) => {
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
                        val: &match axis {
                            None => format!("{:.3}", d),
                            Some(Axis::LeftRight) => format!("H{:+.3}", d),
                            Some(Axis::TopBottom) => format!("V{:+.3}", d),
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
        }
    }

    pub fn equations(&self, drawing: &mut crate::Data) -> Vec<Expression> {
        use Constraint::{Fixed, LineAlongCardinal, LineLength};
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
            LineLength(_, k, d, axis, _) => {
                if let Some(Feature::LineSegment(_, f1, f2)) = drawing.features.get(*k) {
                    let td = &drawing.terms.get_feature_term(*k, TermType::ScalarDistance);
                    let (x1, y1, x2, y2) = (
                        &drawing.terms.get_feature_term(*f1, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f1, TermType::PositionY),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionX),
                        &drawing.terms.get_feature_term(*f2, TermType::PositionY),
                    );

                    match axis {
                        Some(Axis::LeftRight) => vec![
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(Expression::Rational(
                                    Rational::from_float(*d).unwrap(),
                                    true,
                                )),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Variable(x1.into())),
                                    Box::new(Expression::Variable(x2.into())),
                                )),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(y2.into())),
                                Box::new(Expression::Variable(y1.into())),
                            ),
                        ],
                        Some(Axis::TopBottom) => vec![
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(Expression::Rational(
                                    Rational::from_float(*d).unwrap(),
                                    true,
                                )),
                            ),
                            Expression::Equal(
                                Box::new(Expression::Variable(td.into())),
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Variable(y1.into())),
                                    Box::new(Expression::Variable(y2.into())),
                                )),
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
        }
    }
}

fn distance_eq(d: &TermRef, x1: &TermRef, y1: &TermRef, x2: &TermRef, y2: &TermRef) -> Expression {
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
