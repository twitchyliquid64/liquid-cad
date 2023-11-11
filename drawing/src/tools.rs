use super::PaintParams;
use crate::data::Hover;
use crate::handler::ToolResponse;

const TOOL_ICON_SIZE: egui::Vec2 = egui::Vec2 { x: 32.0, y: 32.0 };
const TOOL_ICON_STROKE: f32 = 1.;

fn tool_icon_offsets(idx: usize) -> (f32, f32) {
    let offset_x = 5. + (idx % 2) as f32 * (TOOL_ICON_SIZE.x + 2. * TOOL_ICON_STROKE);
    let offset_y = 5. + (idx / 2) as f32 * (TOOL_ICON_SIZE.y + 2. * TOOL_ICON_STROKE);

    (offset_x, offset_y)
}

fn tool_icon_bounds(rect: egui::Rect, idx: usize) -> egui::Rect {
    let (offset_x, offset_y) = tool_icon_offsets(idx);

    egui::Rect {
        min: egui::Pos2 {
            x: rect.min.x + offset_x + TOOL_ICON_STROKE,
            y: rect.min.y + offset_y + TOOL_ICON_STROKE,
        },
        max: egui::Pos2 {
            x: rect.min.x + offset_x + TOOL_ICON_SIZE.x,
            y: rect.min.y + offset_y + TOOL_ICON_SIZE.y,
        },
    }
}

fn point_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    painter.rect_filled(
        egui::Rect {
            min: c + egui::Vec2 { x: -3., y: -3. },
            max: c + egui::Vec2 { x: 3., y: 3. },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
}

fn line_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    painter.line_segment(
        [
            c + egui::Vec2 { x: -8.5, y: -4.5 },
            c + egui::Vec2 { x: 8.5, y: 4.5 },
        ],
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::WHITE,
        },
    );

    painter.rect_filled(
        egui::Rect {
            min: c + egui::Vec2 { x: -10., y: -6. },
            max: c + egui::Vec2 { x: -7., y: -3. },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
    painter.rect_filled(
        egui::Rect {
            min: c + egui::Vec2 { x: 7., y: 3. },
            max: c + egui::Vec2 { x: 10., y: 6. },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
}

fn fixed_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    let layout = painter.layout_no_wrap(
        "(x,y)".into(),
        egui::FontId::monospace(8.),
        egui::Color32::WHITE,
    );

    painter.galley(
        c + egui::Vec2 {
            x: -layout.rect.width() / 2.,
            y: -layout.rect.height() / 2.,
        },
        layout,
    );
}

fn dim_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    painter.vline(
        c.x - 10.,
        (c.y - 7.)..=(c.y + 7.),
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::LIGHT_BLUE,
        },
    );
    painter.vline(
        c.x + 10.,
        (c.y - 7.)..=(c.y + 7.),
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::LIGHT_BLUE,
        },
    );
    painter.hline(
        (c.x - 9.)..=(c.x + 10.),
        c.y,
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::WHITE,
        },
    );
}

#[derive(Debug, Default, Clone)]
enum Tool {
    #[default]
    Point,
    Line(Option<egui::Pos2>),
    Fixed,
    Dimension,
}

impl Tool {
    pub fn same_tool(&self, other: &Self) -> bool {
        match (self, other) {
            (Tool::Point, Tool::Point) => true,
            (Tool::Line(_), Tool::Line(_)) => true,
            (Tool::Fixed, Tool::Fixed) => true,
            (Tool::Dimension, Tool::Dimension) => true,
            _ => false,
        }
    }

    pub fn all<'a>() -> &'a [Tool] {
        &[Tool::Point, Tool::Line(None), Tool::Fixed, Tool::Dimension]
    }

    pub fn toolbar_size() -> egui::Pos2 {
        let odd_len = if Tool::all().len() % 2 == 0 {
            Tool::all().len() - 1
        } else {
            Tool::all().len()
        };

        egui::Pos2 {
            x: tool_icon_offsets(odd_len).0 + TOOL_ICON_SIZE.x + TOOL_ICON_STROKE,
            y: tool_icon_offsets(odd_len).1 + TOOL_ICON_SIZE.y + TOOL_ICON_STROKE,
        }
    }

    pub fn handle_input(
        &mut self,
        _ui: &mut egui::Ui,
        hp: egui::Pos2,
        hover: &Hover,
        response: &egui::Response,
    ) -> Option<ToolResponse> {
        match self {
            Tool::Point => {
                match (
                    hover,
                    response.clicked(),
                    response.drag_started_by(egui::PointerButton::Primary)
                        || response.drag_released_by(egui::PointerButton::Primary),
                ) {
                    (Hover::None, true, _) => Some(ToolResponse::NewPoint(hp)),
                    (Hover::Feature { .. } | Hover::Constraint { .. }, true, _) => None,
                    (_, _, true) => Some(ToolResponse::Handled), // catch drag events

                    (_, false, false) => None,
                }
            }

            Tool::Line(p1) => {
                let c = match (hover, &p1, response.clicked()) {
                    // No first point, clicked on a point
                    (
                        Hover::Feature {
                            k: _,
                            feature: crate::Feature::Point(_, x, y),
                        },
                        None,
                        true,
                    ) => {
                        *p1 = Some(egui::Pos2 { x: *x, y: *y });
                        Some(ToolResponse::Handled)
                    }
                    // Has first point, clicked on a point
                    (
                        Hover::Feature {
                            k: _,
                            feature: crate::Feature::Point(_, x2, y2),
                        },
                        Some(starting_point),
                        true,
                    ) => {
                        let starting_point = starting_point.clone();
                        *p1 = Some(egui::Pos2 { x: *x2, y: *y2 });
                        Some(ToolResponse::NewLineSegment(
                            starting_point,
                            egui::Pos2 { x: *x2, y: *y2 },
                        ))
                    }
                    (Hover::None, Some(_), true) => {
                        *p1 = None;
                        Some(ToolResponse::Handled)
                    }
                    // No first point, clicked empty space or line
                    (Hover::None, None, true)
                    | (
                        Hover::Feature {
                            feature: crate::Feature::LineSegment(..),
                            ..
                        },
                        None,
                        true,
                    ) => Some(ToolResponse::SwitchToPointer),

                    _ => None,
                };
                if c.is_some() {
                    return c;
                }

                // Intercept drag events.
                if response.drag_started_by(egui::PointerButton::Primary)
                    || response.drag_released_by(egui::PointerButton::Primary)
                {
                    return Some(ToolResponse::Handled);
                }

                None
            }

            Tool::Fixed => {
                if response.clicked() {
                    return match hover {
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Point(..),
                        } => Some(ToolResponse::NewFixedConstraint(k.clone())),
                        _ => Some(ToolResponse::SwitchToPointer),
                    };
                }

                // Intercept drag events.
                if response.drag_started_by(egui::PointerButton::Primary)
                    || response.drag_released_by(egui::PointerButton::Primary)
                {
                    return Some(ToolResponse::Handled);
                }
                None
            }

            Tool::Dimension => {
                if response.clicked() {
                    return match hover {
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        } => Some(ToolResponse::NewLineLengthConstraint(k.clone())),
                        _ => Some(ToolResponse::SwitchToPointer),
                    };
                }

                // Intercept drag events.
                if response.drag_started_by(egui::PointerButton::Primary)
                    || response.drag_released_by(egui::PointerButton::Primary)
                {
                    return Some(ToolResponse::Handled);
                }
                None
            }
        }
    }

    pub fn draw_active(
        &self,
        painter: &egui::Painter,
        response: &egui::Response,
        hp: egui::Pos2,
        params: &PaintParams,
    ) {
        match self {
            Tool::Line(None) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("new line: click 1st point");
            }
            Tool::Line(Some(start)) => {
                painter.line_segment(
                    [params.vp.translate_point(*start), hp],
                    egui::Stroke {
                        width: TOOL_ICON_STROKE,
                        color: egui::Color32::WHITE,
                    },
                );

                response
                    .clone()
                    .on_hover_text_at_pointer("new line: click 2nd point");
            }

            Tool::Point => {
                response.clone().on_hover_text_at_pointer("new point");
            }

            Tool::Fixed => {
                response.clone().on_hover_text_at_pointer("constrain (x,y)");
            }

            Tool::Dimension => {
                response.clone().on_hover_text_at_pointer("constrain d");
            }
        }
    }

    fn icon_painter(&self) -> impl FnOnce(egui::Rect, &egui::Painter) {
        match self {
            Tool::Point => point_tool_icon,
            Tool::Line(_) => line_tool_icon,
            Tool::Fixed => fixed_tool_icon,
            Tool::Dimension => dim_tool_icon,
        }
    }

    pub fn paint_icon(
        &self,
        painter: &egui::Painter,
        hp: Option<egui::Pos2>,
        params: &PaintParams,
        selected: bool,
        idx: usize,
    ) -> egui::Rect {
        let bounds = tool_icon_bounds(params.rect, idx);

        let hovered = hp.map(|hp| bounds.contains(hp)).unwrap_or(false);

        if selected {
            painter.rect_filled(
                bounds.shrink(TOOL_ICON_STROKE),
                egui::Rounding::ZERO,
                if hovered {
                    params.colors.text
                } else {
                    params.colors.text.gamma_multiply(0.5)
                },
            );
        } else if hovered {
            painter.rect_filled(
                bounds.shrink(TOOL_ICON_STROKE),
                egui::Rounding::ZERO,
                params.colors.text,
            );
        }

        self.icon_painter()(bounds, painter);

        bounds
    }
}

#[derive(Debug, Default)]
pub struct Toolbar {
    current: Option<Tool>,
}

impl Toolbar {
    pub fn clear(&mut self) {
        self.current = None;
    }

    pub fn handle_input(
        &mut self,
        ui: &mut egui::Ui,
        hp: Option<egui::Pos2>,
        hover: &Hover,
        response: &egui::Response,
    ) -> Option<ToolResponse> {
        // Escape to exit use of a tool
        if self.current.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.current = None;
            return Some(ToolResponse::Handled);
        }

        // Hotkeys for switching tools
        if response.has_focus() && !response.dragged() {
            let (l, p, s, d) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::L),
                    i.key_pressed(egui::Key::P),
                    i.key_pressed(egui::Key::S),
                    i.key_pressed(egui::Key::D),
                )
            });
            match (l, p, s, d) {
                (true, _, _, _) => {
                    self.current = Some(Tool::Line(None));
                    return Some(ToolResponse::Handled);
                }
                (_, true, _, _) => {
                    self.current = Some(Tool::Point);
                    return Some(ToolResponse::Handled);
                }
                (_, _, true, _) => {
                    self.current = Some(Tool::Fixed);
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, true) => {
                    self.current = Some(Tool::Dimension);
                    return Some(ToolResponse::Handled);
                }
                _ => {}
            }
        }

        if let (Some(hp), true) = (
            hp,
            response.clicked()
                || response.dragged()
                || response.drag_started()
                || response.drag_released(),
        ) {
            for (i, tool) in Tool::all().iter().enumerate() {
                let bounds = tool_icon_bounds(response.rect, i);
                if bounds.contains(hp) {
                    if response.clicked() {
                        self.current = Some(tool.clone());
                    }
                    return Some(ToolResponse::Handled);
                }
            }

            if let Some(current) = self.current.as_mut() {
                return current.handle_input(ui, hp, hover, response);
            }
        }
        None
    }

    pub fn paint(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        response: &egui::Response,
        hp: Option<egui::Pos2>,
        params: &PaintParams,
    ) {
        painter.rect_filled(
            egui::Rect {
                min: egui::Pos2 {
                    x: params.rect.min.x + tool_icon_offsets(0).0,
                    y: params.rect.min.y + tool_icon_offsets(0).1,
                },
                max: egui::Pos2 {
                    x: params.rect.min.x + Tool::toolbar_size().x,
                    y: params.rect.min.y + Tool::toolbar_size().y,
                },
            },
            egui::Rounding::ZERO,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        );

        for (i, tool) in Tool::all().iter().enumerate() {
            let active = self
                .current
                .as_ref()
                .map(|t| t.same_tool(tool))
                .unwrap_or(false);
            tool.paint_icon(painter, hp, params, active, i);
        }

        if let (Some(hp), Some(tool)) = (hp, self.current.as_ref()) {
            tool.draw_active(painter, response, hp, params);
        }
    }
}
