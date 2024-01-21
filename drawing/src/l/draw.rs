const ARROW_MARGIN: f32 = 2.0;

fn arrow(
    from: egui::Pos2,
    to: egui::Pos2,
    width: f32,
    stroke: egui::Stroke,
    painter: &egui::Painter,
) {
    let t = (from - to).angle();
    let margin = egui::Vec2::angled(t) * ARROW_MARGIN;
    let arrow_base = to + (egui::Vec2::angled(t) * width * 3.0) + margin;

    painter.line_segment([from, to + margin], stroke);
    painter.add(egui::Shape::convex_polygon(
        vec![
            to + margin + egui::Vec2::angled(t),
            arrow_base + (egui::Vec2::angled(t - std::f32::consts::PI / 2.0) * width),
            arrow_base + (egui::Vec2::angled(t + std::f32::consts::PI / 2.0) * width),
        ],
        stroke.color,
        stroke,
    ));
}

// reference dimensions are in drawing-space.
pub struct DimensionLengthOverlay<'a> {
    pub val: &'a str,
    pub a: egui::Pos2,
    pub b: egui::Pos2,
    pub reference: egui::Vec2,
    pub hovered: bool,
    pub selected: bool,
    pub arrow_fill: bool,
}

impl<'a> DimensionLengthOverlay<'a> {
    const LINE_STOP_OFFSET: f32 = 8.5;
    const TEXT_MARGIN: egui::Vec2 = egui::Vec2 { x: 10.0, y: 2.0 };

    pub fn draw(&self, painter: &egui::Painter, params: &crate::PaintParams) {
        let vp = &params.vp;
        let angle = (self.a - self.b).angle() + self.reference.angle();
        let (sa, sb) = (vp.translate_point(self.a), vp.translate_point(self.b));

        let stop_l = egui::Vec2::angled(std::f32::consts::PI / 2.).dot(self.reference);
        let stop_angle = angle - self.reference.angle() + std::f32::consts::PI / 2.;

        let color = if self.selected {
            params.colors.selected
        } else if self.hovered {
            params.colors.hover
        } else {
            egui::Color32::LIGHT_BLUE
        };

        self.draw_stop_lines(stop_l, stop_angle, sa, sb, painter, color);

        let layout = painter.layout_no_wrap(self.val.into(), egui::FontId::monospace(10.), color);
        let text_pos = vp.translate_point(self.a.lerp(self.b, 0.5))
            + egui::Vec2::angled(angle) * self.reference.length();

        self.draw_parallel_arrows(
            angle,
            stop_l,
            stop_angle,
            sa,
            sb,
            text_pos,
            &layout.rect,
            color,
            painter,
        );
        painter.galley(
            text_pos
                - egui::Vec2 {
                    x: layout.rect.width() / 2.,
                    y: layout.rect.height() / 2.,
                },
            layout,
        );
    }

    #[inline]
    fn draw_parallel_arrows(
        &self,
        angle: f32,
        stop_l: f32,
        stop_angle: f32,
        sa: egui::Pos2,
        sb: egui::Pos2,
        text_pos: egui::Pos2,
        text_bounds: &egui::Rect,
        color: egui::Color32,
        painter: &egui::Painter,
    ) {
        let v = egui::Vec2::angled(angle) * self.reference.length();
        let text_offset = text_pos.to_vec2()
            - egui::Vec2 {
                x: text_bounds.width() / 2.,
                y: text_bounds.height() / 2.,
            };

        let arrow_line_1 = crate::l::LineSegment {
            p1: sa + v,
            p2: text_pos,
        };
        let arrow_line_2 = crate::l::LineSegment {
            p1: text_pos,
            p2: sb + v,
        };

        let end_1 = arrow_line_1.intersection_rect(
            &text_bounds
                .expand2(Self::TEXT_MARGIN)
                .translate(text_offset),
        );
        let end_2 = arrow_line_2.intersection_rect(
            &text_bounds
                .expand2(Self::TEXT_MARGIN)
                .translate(text_offset),
        );

        match (end_1, end_2, self.arrow_fill) {
            (Some(e1), Some(e2), false)
                if arrow_line_1.p1.distance_sq(e1) > 750.
                    && arrow_line_2.p1.distance_sq(e2) > 750. =>
            {
                painter.arrow(
                    e1,
                    egui::Vec2::angled((sa - sb).angle()) * 20.,
                    egui::Stroke { width: 1., color },
                );
                painter.arrow(
                    e2,
                    egui::Vec2::angled((sb - sa).angle()) * 20.,
                    egui::Stroke { width: 1., color },
                );
            }

            (Some(e1), Some(e2), true) => {
                let s = egui::Stroke { width: 1., color };
                let w = 2.;
                arrow(
                    e1,
                    sa + egui::Vec2::angled(stop_angle) * stop_l,
                    w,
                    s,
                    painter,
                );
                arrow(
                    e2,
                    sb + egui::Vec2::angled(stop_angle) * stop_l,
                    w,
                    s,
                    painter,
                );
            }
            _ => {}
        }
    }

    fn draw_stop_lines(
        &self,
        l: f32,
        t: f32,
        sa: egui::Pos2,
        sb: egui::Pos2,
        painter: &egui::Painter,
        color: egui::Color32,
    ) {
        let offset = if l >= 0. {
            DimensionLengthOverlay::LINE_STOP_OFFSET
        } else {
            -DimensionLengthOverlay::LINE_STOP_OFFSET
        };

        painter.line_segment(
            [
                sa + egui::Vec2::angled(t) * l,
                sa + egui::Vec2::angled(t) * offset,
            ],
            egui::Stroke { width: 1., color },
        );
        painter.line_segment(
            [
                sb + egui::Vec2::angled(t) * l,
                sb + egui::Vec2::angled(t) * offset,
            ],
            egui::Stroke { width: 1., color },
        );
    }
}

// all input dimensions are in drawing-space.
pub struct DimensionRadiusOverlay<'a> {
    pub val: &'a str,
    pub center: egui::Pos2,
    pub radius: &'a f32,
    pub reference: egui::Vec2,
    pub hovered: bool,
    pub selected: bool,
}

impl<'a> DimensionRadiusOverlay<'a> {
    pub fn draw(&self, painter: &egui::Painter, params: &crate::PaintParams) {
        let vp = &params.vp;
        let r_scaled = *self.radius / vp.zoom;
        let center = vp.translate_point(self.center);

        let color = if self.selected {
            params.colors.selected
        } else if self.hovered {
            params.colors.hover
        } else {
            egui::Color32::LIGHT_BLUE
        };
        let layout = painter.layout_no_wrap(self.val.into(), egui::FontId::monospace(10.), color);
        let text_offset = center + self.reference;

        if self.reference.length() > r_scaled {
            let intercept: egui::Pos2 =
                center + (egui::Vec2::angled(self.reference.angle()) * r_scaled);

            let dim_line = crate::l::LineSegment {
                p1: intercept,
                p2: text_offset,
            };
            if let Some(end) = dim_line.intersection_rect(
                &layout.rect.expand2((10., 2.).into()).translate(
                    text_offset
                        - egui::Vec2 {
                            x: layout.rect.width() / 2.,
                            y: layout.rect.height() / 2.,
                        }
                        .to_pos2(),
                ),
            ) {
                arrow(
                    end,
                    intercept,
                    2.0,
                    egui::Stroke { width: 1., color },
                    painter,
                );
            }
        }

        painter.galley(
            text_offset
                - egui::Vec2 {
                    x: layout.rect.width() / 2.,
                    y: layout.rect.height() / 2.,
                },
            layout,
        );
    }
}

const TICK_SIZE: f32 = 4.0;
const TICK_SPACING: f32 = 5.0;

pub fn length_tick(
    a: egui::Pos2,
    b: egui::Pos2,
    ticks: usize,
    painter: &egui::Painter,
    params: &crate::PaintParams,
) {
    let (a, b) = (params.vp.translate_point(a), params.vp.translate_point(b));
    let t = (a - b).angle();
    let start = b.lerp(a, 0.3)
        + ((ticks as f32 - 1.0) * egui::Vec2::angled((b - a).angle()) * TICK_SPACING) / 2.0;

    for i in 0..=ticks {
        let along = i as f32 * egui::Vec2::angled(t) * TICK_SPACING;

        painter.line_segment(
            [
                start + along + egui::Vec2::angled(t - std::f32::consts::PI / 2.0) * TICK_SIZE,
                start + along + egui::Vec2::angled(t + std::f32::consts::PI / 2.0) * TICK_SIZE,
            ],
            egui::Stroke {
                width: 1.,
                color: egui::Color32::LIGHT_BLUE,
            },
        );
    }
}
