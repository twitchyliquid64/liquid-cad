// reference dimensions are in drawing-space.
pub struct DimensionLengthOverlay<'a> {
    pub val: &'a str,
    pub a: egui::Pos2,
    pub b: egui::Pos2,
    pub reference: egui::Vec2,
    pub hovered: bool,
    pub selected: bool,
}

impl<'a> DimensionLengthOverlay<'a> {
    const LINE_STOP_OFFSET: f32 = 8.5;

    pub fn draw(&self, painter: &egui::Painter, params: &crate::PaintParams) {
        let vp = &params.vp;
        let t = (self.a - self.b).angle() + self.reference.angle();
        let (sa, sb) = (vp.translate_point(self.a), vp.translate_point(self.b));

        let color = if self.selected {
            params.colors.selected
        } else if self.hovered {
            params.colors.hover
        } else {
            egui::Color32::LIGHT_BLUE
        };

        self.draw_stop_lines(t, sa, sb, painter, color);

        let layout = painter.layout_no_wrap(self.val.into(), egui::FontId::monospace(10.), color);
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

        let arrow_line_1 = crate::l::LineSegment {
            p1: sa + v,
            p2: text_pos,
        };
        if let Some(end) = arrow_line_1
            .intersection_rect(&text_bounds.expand2((10., 2.).into()).translate(text_offset))
        {
            if arrow_line_1.p1.distance_sq(end) > 750. {
                painter.arrow(
                    end,
                    egui::Vec2::angled((sa - sb).angle()) * 20.,
                    egui::Stroke { width: 1., color },
                );
            }
        }

        let arrow_line_2 = crate::l::LineSegment {
            p1: text_pos,
            p2: sb + v,
        };
        if let Some(end) = arrow_line_2
            .intersection_rect(&text_bounds.expand2((10., 2.).into()).translate(text_offset))
        {
            if arrow_line_2.p2.distance_sq(end) > 750. {
                painter.arrow(
                    end,
                    egui::Vec2::angled((sb - sa).angle()) * 20.,
                    egui::Stroke { width: 1., color },
                );
            }
        }
    }

    fn draw_stop_lines(
        &self,
        t: f32,
        sa: egui::Pos2,
        sb: egui::Pos2,
        painter: &egui::Painter,
        color: egui::Color32,
    ) {
        let l = egui::Vec2::angled(std::f32::consts::PI / 2.).dot(self.reference);
        let t = t - self.reference.angle() + std::f32::consts::PI / 2.;

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
                painter.line_segment([intercept, end], egui::Stroke { width: 1., color });
                // TODO: Arrow?
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
