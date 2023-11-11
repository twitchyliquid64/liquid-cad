use crate::l::LineSegment;
use crate::{Feature, FeatureKey};

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

                    DimensionLengthOverlay {
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

// all input dimensions are in drawing-space.
struct DimensionLengthOverlay<'a> {
    val: &'a f32,
    a: egui::Pos2,
    b: egui::Pos2,
    reference: egui::Vec2,
    hovered: bool,
    selected: bool,
}

impl<'a> DimensionLengthOverlay<'a> {
    const LINE_STOP_OFFSET: f32 = 5.5;

    pub fn draw(&self, painter: &egui::Painter, params: &crate::PaintParams) {
        let vp = &params.vp;
        let t = (self.a - self.b).angle() + self.reference.angle();
        let (sa, sb) = (vp.translate_point(self.a), vp.translate_point(self.b));

        self.draw_stop_lines(t, sa, sb, painter);

        let color = if self.selected {
            params.colors.selected
        } else if self.hovered {
            params.colors.hover
        } else {
            egui::Color32::LIGHT_BLUE
        };

        let layout = painter.layout_no_wrap(
            format!("{:.3}", self.val).into(),
            egui::FontId::monospace(10.),
            color,
        );
        let text_pos = vp.translate_point(self.a.lerp(self.b, 0.5))
            + egui::Vec2::angled(t) * self.reference.length();

        self.draw_parallel_arrows(t, sa, sb, text_pos, &layout.rect, color, painter);
        painter.galley(
            text_pos
                - egui::Vec2 {
                    x: layout.rect.width() / 2.,
                    y: layout.rect.height() / 2.,
                },
            layout,
        );
    }

    fn draw_parallel_arrows(
        &self,
        t: f32,
        sa: egui::Pos2,
        sb: egui::Pos2,
        text_pos: egui::Pos2,
        text_bounds: &egui::Rect,
        color: egui::Color32,
        painter: &egui::Painter,
    ) {
        let v = egui::Vec2::angled(t) * self.reference.length();
        let text_offset = text_pos.to_vec2()
            - egui::Vec2 {
                x: text_bounds.width() / 2.,
                y: text_bounds.height() / 2.,
            };

        let arrow_line_1 = LineSegment {
            p1: sa + v,
            p2: text_pos,
        };
        if let Some(end) = arrow_line_1
            .intersection_rect(&text_bounds.expand2((12., 2.).into()).translate(text_offset))
        {
            if sa.distance_sq(end) > 1950. {
                painter.arrow(
                    end,
                    egui::Vec2::angled((sa - sb).angle()) * 20.,
                    egui::Stroke { width: 1., color },
                );
            }
        }

        let arrow_line_2 = LineSegment {
            p1: text_pos,
            p2: sb + v,
        };
        if let Some(end) = arrow_line_2
            .intersection_rect(&text_bounds.expand2((12., 2.).into()).translate(text_offset))
        {
            if sb.distance_sq(end) > 1950. {
                painter.arrow(
                    end,
                    egui::Vec2::angled((sb - sa).angle()) * 20.,
                    egui::Stroke { width: 1., color },
                );
            }
        }
    }

    fn draw_stop_lines(&self, t: f32, sa: egui::Pos2, sb: egui::Pos2, painter: &egui::Painter) {
        let l = self.reference.length();

        painter.line_segment(
            [
                sa + egui::Vec2::angled(t) * l,
                sa + egui::Vec2::angled(t) * DimensionLengthOverlay::LINE_STOP_OFFSET,
            ],
            egui::Stroke {
                width: 1.,
                color: egui::Color32::LIGHT_BLUE,
            },
        );
        painter.line_segment(
            [
                sb + egui::Vec2::angled(t) * l,
                sb + egui::Vec2::angled(t) * DimensionLengthOverlay::LINE_STOP_OFFSET,
            ],
            egui::Stroke {
                width: 1.,
                color: egui::Color32::LIGHT_BLUE,
            },
        );
    }
}
