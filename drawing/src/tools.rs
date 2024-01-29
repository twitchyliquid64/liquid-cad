use super::PaintParams;
use crate::data::Hover;
use crate::handler::ToolResponse;
use crate::FeatureKey;

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

fn horizontal_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    let layout = painter.layout_no_wrap(
        "H".into(),
        egui::FontId::monospace(10.),
        egui::Color32::LIGHT_BLUE,
    );

    painter.galley(
        c + egui::Vec2 {
            x: -layout.rect.width() / 2.,
            y: -layout.rect.height() / 2.,
        },
        layout,
    );
}

fn vertical_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    let layout = painter.layout_no_wrap(
        "V".into(),
        egui::FontId::monospace(10.),
        egui::Color32::LIGHT_BLUE,
    );

    painter.galley(
        c + egui::Vec2 {
            x: -layout.rect.width() / 2.,
            y: -layout.rect.height() / 2.,
        },
        layout,
    );
}

fn lerp_tool_icon(b: egui::Rect, painter: &egui::Painter) {
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
            min: c + egui::Vec2 { x: -1.5, y: -1.5 },
            max: c + egui::Vec2 { x: 1.5, y: 1.5 },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
}

fn equal_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    painter.line_segment(
        [
            c + egui::Vec2 { x: 8.5, y: -4.5 },
            c + egui::Vec2 { x: -8.5, y: 4.5 },
        ],
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::WHITE,
        },
    );
    painter.line_segment(
        [
            c + egui::Vec2 { x: -1.5, y: -1.5 },
            c + egui::Vec2 { x: 1.5, y: 1.5 },
        ],
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::LIGHT_RED,
        },
    );
}

fn arc_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();

    let shape = egui::epaint::CubicBezierShape::from_points_stroke(
        [
            c + egui::Vec2 { x: -8.5, y: -4.5 },
            c + egui::Vec2 { x: -5.5, y: -9.0 },
            c + egui::Vec2 { x: 8.5, y: -9.0 },
            c + egui::Vec2 { x: 8.5, y: 4.5 },
        ],
        false,
        egui::Color32::TRANSPARENT,
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::WHITE,
        },
    );
    painter.add(shape);

    painter.rect_filled(
        egui::Rect {
            min: c + egui::Vec2 { x: -8.5, y: -4.5 } + egui::Vec2 { x: -1.5, y: -1.5 },
            max: c + egui::Vec2 { x: -8.5, y: -4.5 } + egui::Vec2 { x: 1.5, y: 1.5 },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
    painter.rect_filled(
        egui::Rect {
            min: c + egui::Vec2 { x: 8.5, y: 4.5 } + egui::Vec2 { x: -1.5, y: -1.5 },
            max: c + egui::Vec2 { x: 8.5, y: 4.5 } + egui::Vec2 { x: 1.5, y: 1.5 },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
}

fn circle_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();

    painter.circle_stroke(
        c,
        8.5,
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::WHITE,
        },
    );
    painter.rect_filled(
        egui::Rect {
            min: c + egui::Vec2 { x: -1.5, y: -1.5 },
            max: c + egui::Vec2 { x: 1.5, y: 1.5 },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
    painter.rect_filled(
        egui::Rect {
            min: c + egui::Vec2 { x: 8.5, y: 0.0 } + egui::Vec2 { x: -1.5, y: -1.5 },
            max: c + egui::Vec2 { x: 8.5, y: 0.0 } + egui::Vec2 { x: 1.5, y: 1.5 },
        },
        egui::Rounding::ZERO,
        egui::Color32::GREEN,
    );
}

fn parallel_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    painter.line_segment(
        [
            c + egui::Vec2 { x: 8.5, y: -4.5 },
            c + egui::Vec2 { x: -8.5, y: 4.5 },
        ],
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::WHITE,
        },
    );
    painter.line_segment(
        [
            c + egui::Vec2 { x: 8.5, y: -4.5 } + egui::Vec2 { x: 0., y: -2.5 },
            c + egui::Vec2 { x: -8.5, y: 4.5 } + egui::Vec2 { x: 0., y: -2.5 },
        ],
        egui::Stroke {
            width: TOOL_ICON_STROKE,
            color: egui::Color32::WHITE,
        },
    );
}

fn angle_tool_icon(b: egui::Rect, painter: &egui::Painter) {
    let c = b.center();
    let layout = painter.layout_no_wrap(
        "SIN".into(),
        egui::FontId::monospace(8.),
        egui::Color32::LIGHT_BLUE,
    );

    painter.galley(
        c + egui::Vec2 {
            x: -layout.rect.width() / 2.,
            y: -layout.rect.height() / 2.,
        },
        layout,
    );
}

#[derive(Debug, Default, Clone)]
enum Tool {
    #[default]
    Point,
    Line(Option<FeatureKey>),
    Arc(Option<FeatureKey>),
    Circle(Option<FeatureKey>),
    Fixed,
    Dimension,
    Horizontal,
    Vertical,
    Lerp(Option<FeatureKey>),
    Equal(Option<FeatureKey>),
    Parallel(Option<FeatureKey>),
    Angle,
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Point => "Create Point",
            Tool::Line(_) => "Create Line",
            Tool::Arc(_) => "Create Arc",
            Tool::Circle(_) => "Create Circle",
            Tool::Fixed => "Constrain to co-ords",
            Tool::Dimension => "Constrain length/radius",
            Tool::Horizontal => "Constrain horizontal",
            Tool::Vertical => "Constrain vertical",
            Tool::Lerp(_) => "Constrain point along line",
            Tool::Equal(_) => "Constrain equal",
            Tool::Parallel(_) => "Constrain lines as parallel",
            Tool::Angle => "Constain line angle",
        }
    }
    pub fn key(&self) -> Option<&'static str> {
        match self {
            Tool::Point => Some("P"),
            Tool::Line(_) => Some("L"),
            Tool::Arc(_) => Some("R"),
            Tool::Circle(_) => Some("C"),
            Tool::Fixed => Some("S"),
            Tool::Dimension => Some("D"),
            Tool::Horizontal => Some("H"),
            Tool::Vertical => Some("V"),
            Tool::Lerp(_) => Some("I"),
            Tool::Equal(_) => Some("E"),
            Tool::Parallel(_) => None,
            Tool::Angle => Some("N"),
        }
    }
    pub fn long_tooltip(&self) -> Option<&'static str> {
        match self {
            Tool::Point => Some("Creates points.\n\nClick anywhere in free space to create a point."),
            Tool::Line(_) => Some("Creates lines from existing points.\n\nClick on the first point and then the second to create a line."),
            Tool::Arc(_) => Some("Creates a circular arc between points.\n\nClick on the first point and then the second to create an arc. A center point will be automatically created."),
            Tool::Circle(_) => Some("Creates a circle around some center point.\n\nClick on the center point, and then again in empty space to create the circle."),
            Tool::Fixed => Some("Constraints a point to be at specific co-ordinates.\n\nClick a point to constrain it to (0,0). Co-ordinates can be changed later in the selection UI."),
            Tool::Dimension => Some("Sets the dimensions of a line or circle.\n\nClick a line/circle to constrain it to its current length/radius respectively. The constrained value can be changed later in the selection UI."),
            Tool::Horizontal => Some("Constrains a line to be horizontal."),
            Tool::Vertical => Some("Constrains a line to be vertical."),
            Tool::Lerp(_) => Some("Constrains a point to be a certain percentage along a line.\n\nClick a point, and then its corresponding line to apply this constraint. The percentage defaults to 50% but can be changed later in the selection UI."),
            Tool::Equal(_) => Some("Constrains a line/circle to be equal in length/radius to another line/circle."),
            Tool::Parallel(_) => Some("Constrains a line to be parallel to another line.\n\nWARNING: THIS TOOL IS EXPERIMENTAL and not working properly.\n\nClick on the first line, and then the second line to create this constraint."),
            Tool::Angle => Some("Constrains a line to have some angle clockwise from the vertical axis."),
        }
    }

    pub fn same_tool(&self, other: &Self) -> bool {
        match (self, other) {
            (Tool::Point, Tool::Point) => true,
            (Tool::Line(_), Tool::Line(_)) => true,
            (Tool::Arc(_), Tool::Arc(_)) => true,
            (Tool::Circle(_), Tool::Circle(_)) => true,
            (Tool::Fixed, Tool::Fixed) => true,
            (Tool::Dimension, Tool::Dimension) => true,
            (Tool::Horizontal, Tool::Horizontal) => true,
            (Tool::Vertical, Tool::Vertical) => true,
            (Tool::Lerp(_), Tool::Lerp(_)) => true,
            (Tool::Equal(_), Tool::Equal(_)) => true,
            (Tool::Parallel(_), Tool::Parallel(_)) => true,
            (Tool::Angle, Tool::Angle) => true,
            _ => false,
        }
    }

    pub fn all<'a>() -> &'a [Tool] {
        &[
            Tool::Point,
            Tool::Line(None),
            Tool::Circle(None),
            Tool::Arc(None),
            Tool::Fixed,
            Tool::Dimension,
            Tool::Horizontal,
            Tool::Vertical,
            Tool::Lerp(None),
            Tool::Equal(None),
            Tool::Parallel(None),
            Tool::Angle,
        ]
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
                            k,
                            feature: crate::Feature::Point(..),
                        },
                        None,
                        true,
                    ) => {
                        *p1 = Some(*k);
                        Some(ToolResponse::Handled)
                    }
                    // Has first point, clicked on a point
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Point(..),
                        },
                        Some(starting_point),
                        true,
                    ) => {
                        let starting_point = starting_point.clone();
                        *p1 = Some(*k);
                        Some(ToolResponse::NewLineSegment(starting_point, *k))
                    }
                    (Hover::None, Some(_), true) => {
                        *p1 = None;
                        Some(ToolResponse::Handled)
                    }
                    // No first point, clicked empty space or line or arc or circle
                    (Hover::None, None, true)
                    | (
                        Hover::Feature {
                            feature: crate::Feature::LineSegment(..),
                            ..
                        },
                        None,
                        true,
                    )
                    | (
                        Hover::Feature {
                            feature: crate::Feature::Arc(..),
                            ..
                        },
                        None,
                        true,
                    )
                    | (
                        Hover::Feature {
                            feature: crate::Feature::Circle(..),
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

            Tool::Arc(p1) => {
                let c = match (hover, &p1, response.clicked()) {
                    // No first point, clicked on a point
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Point(..),
                        },
                        None,
                        true,
                    ) => {
                        *p1 = Some(*k);
                        Some(ToolResponse::Handled)
                    }
                    // Has first point, clicked on a point
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Point(..),
                        },
                        Some(starting_point),
                        true,
                    ) => {
                        let starting_point = starting_point.clone();
                        *p1 = Some(*k);
                        Some(ToolResponse::NewArc(starting_point, *k))
                    }
                    (Hover::None, Some(_), true) => {
                        *p1 = None;
                        Some(ToolResponse::Handled)
                    }
                    // No first point, clicked empty space or line or arc or circle
                    (Hover::None, None, true)
                    | (
                        Hover::Feature {
                            feature: crate::Feature::LineSegment(..),
                            ..
                        },
                        None,
                        true,
                    )
                    | (
                        Hover::Feature {
                            feature: crate::Feature::Arc(..),
                            ..
                        },
                        None,
                        true,
                    )
                    | (
                        Hover::Feature {
                            feature: crate::Feature::Circle(..),
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

            Tool::Circle(p1) => {
                let c = match (hover, &p1, response.clicked()) {
                    // No first point, clicked on a point
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Point(..),
                        },
                        None,
                        true,
                    ) => {
                        *p1 = Some(*k);
                        Some(ToolResponse::Handled)
                    }
                    // Has first point, clicked anywhere
                    (_, Some(starting_point), true) => {
                        let starting_point = starting_point.clone();
                        Some(ToolResponse::NewCircle(starting_point, hp))
                    }

                    // No first point, clicked empty space or line or arc
                    (Hover::None, None, true)
                    | (
                        Hover::Feature {
                            feature: crate::Feature::LineSegment(..),
                            ..
                        },
                        None,
                        true,
                    )
                    | (
                        Hover::Feature {
                            feature: crate::Feature::Arc(..),
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
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Circle(..),
                        } => Some(ToolResponse::NewCircleRadiusConstraint(k.clone())),
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

            Tool::Horizontal => {
                if response.clicked() {
                    return match hover {
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        } => Some(ToolResponse::NewLineCardinalConstraint(k.clone(), true)),
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
            Tool::Vertical => {
                if response.clicked() {
                    return match hover {
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        } => Some(ToolResponse::NewLineCardinalConstraint(k.clone(), false)),
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

            Tool::Lerp(p1) => {
                let c = match (hover, &p1, response.clicked()) {
                    // No first point, clicked on a point
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Point(..),
                        },
                        None,
                        true,
                    ) => {
                        *p1 = Some(*k);
                        Some(ToolResponse::Handled)
                    }
                    // Has first point, clicked on a line
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        },
                        Some(starting_point),
                        true,
                    ) => {
                        let starting_point = starting_point.clone();
                        *p1 = None;
                        Some(ToolResponse::NewPointLerp(starting_point, *k))
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

            Tool::Equal(l1) => {
                let c = match (hover, &l1, response.clicked()) {
                    // No first feature, clicked on a line
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        },
                        None,
                        true,
                    ) => {
                        *l1 = Some(*k);
                        Some(ToolResponse::Handled)
                    }
                    // Has first line, clicked on a line
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        },
                        Some(starting_line),
                        true,
                    ) => {
                        let starting_line = starting_line.clone();
                        *l1 = None;
                        Some(ToolResponse::NewEqual(starting_line, *k))
                    }
                    // No first feature, clicked on a circle
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Circle(..),
                        },
                        None,
                        true,
                    ) => {
                        *l1 = Some(*k);
                        Some(ToolResponse::Handled)
                    }
                    // Has first circle, clicked on a circle
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::Circle(..),
                        },
                        Some(starting_circle),
                        true,
                    ) => {
                        let starting_circle = starting_circle.clone();
                        *l1 = None;
                        Some(ToolResponse::NewEqual(starting_circle, *k))
                    }
                    (Hover::None, Some(_), true) => {
                        *l1 = None;
                        Some(ToolResponse::Handled)
                    }
                    // No first feature, clicked empty space or point
                    (Hover::None, None, true)
                    | (
                        Hover::Feature {
                            feature: crate::Feature::Point(..),
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

            Tool::Parallel(l1) => {
                let c = match (hover, &l1, response.clicked()) {
                    // No first line, clicked on a line
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        },
                        None,
                        true,
                    ) => {
                        *l1 = Some(*k);
                        Some(ToolResponse::Handled)
                    }
                    // Has first line, clicked on a line
                    (
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        },
                        Some(starting_line),
                        true,
                    ) => {
                        let starting_line = starting_line.clone();
                        *l1 = None;
                        Some(ToolResponse::NewParallelLine(starting_line, *k))
                    }
                    (Hover::None, Some(_), true) => {
                        *l1 = None;
                        Some(ToolResponse::Handled)
                    }
                    // No first line, clicked empty space or point
                    (Hover::None, None, true)
                    | (
                        Hover::Feature {
                            feature: crate::Feature::Point(..),
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

            Tool::Angle => {
                if response.clicked() {
                    return match hover {
                        Hover::Feature {
                            k,
                            feature: crate::Feature::LineSegment(..),
                        } => Some(ToolResponse::NewGlobalAngleConstraint(k.clone())),
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
        drawing: &crate::Data,
    ) {
        match self {
            Tool::Line(None) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("new line: click 1st point");
            }
            Tool::Line(Some(fk)) => {
                let p = drawing.features.get(*fk).unwrap();
                let (x, y) = match p {
                    crate::Feature::Point(_, x1, y1) => (*x1, *y1),
                    _ => unreachable!(),
                };

                painter.line_segment(
                    [params.vp.translate_point((x, y).into()), hp],
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

            Tool::Arc(None) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("new arc: click start point");
            }
            Tool::Arc(Some(fk)) => {
                let p = drawing.features.get(*fk).unwrap();
                let start: egui::Pos2 = match p {
                    crate::Feature::Point(_, x1, y1) => {
                        params.vp.translate_point((*x1, *y1).into())
                    }
                    _ => unreachable!(),
                };
                let end = hp;
                let center = start.lerp(end, 0.5);
                let r = (start.distance(center) as f64, end.distance(center) as f64);

                let a = kurbo::Arc::from_svg_arc(&kurbo::SvgArc {
                    from: (start.x as f64, start.y as f64).into(),
                    to: (end.x as f64, end.y as f64).into(),
                    radii: r.into(),
                    sweep: true,
                    x_rotation: 0.0,
                    large_arc: {
                        let (d_start, d_end) = (start - center, end - center);
                        let dcross = d_start.x * d_end.y - d_end.x * d_start.y;
                        dcross < 0.0
                    },
                });

                if let Some(a) = a {
                    let mut last = (start.x, start.y);
                    a.to_cubic_beziers(0.1, |p1, p2, p| {
                        let shape = egui::epaint::CubicBezierShape::from_points_stroke(
                            [
                                last.into(),
                                (p1.x as f32, p1.y as f32).into(),
                                (p2.x as f32, p2.y as f32).into(),
                                (p.x as f32, p.y as f32).into(),
                            ],
                            false,
                            egui::Color32::TRANSPARENT,
                            egui::Stroke {
                                width: TOOL_ICON_STROKE,
                                color: egui::Color32::WHITE,
                            },
                        );
                        painter.add(shape);
                        last = (p.x as f32, p.y as f32);
                    });
                } else {
                    painter.debug_text(
                        start.lerp(end, 0.5),
                        egui::Align2::CENTER_CENTER,
                        egui::Color32::DARK_RED,
                        "arc :/",
                    );
                }

                response
                    .clone()
                    .on_hover_text_at_pointer("new arc: click end point");
            }

            Tool::Circle(None) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("new circle: click center point");
            }
            Tool::Circle(Some(fk)) => {
                let p = drawing.features.get(*fk).unwrap();
                let (x, y) = match p {
                    crate::Feature::Point(_, x1, y1) => (*x1, *y1),
                    _ => unreachable!(),
                };
                let c: egui::Pos2 = (x, y).into();

                painter.circle_stroke(
                    params.vp.translate_point(c),
                    c.distance(params.vp.screen_to_point(hp)) / params.vp.zoom,
                    egui::Stroke {
                        width: TOOL_ICON_STROKE,
                        color: egui::Color32::WHITE,
                    },
                );

                response
                    .clone()
                    .on_hover_text_at_pointer("new circle: click to set radius");
            }

            Tool::Fixed => {
                response.clone().on_hover_text_at_pointer("constrain (x,y)");
            }

            Tool::Dimension => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain dimension: click line or circle");
            }
            Tool::Horizontal => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain horizontal");
            }
            Tool::Vertical => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain vertical");
            }

            Tool::Lerp(None) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain lerp: click point");
            }
            Tool::Lerp(Some(_)) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain lerp: click line");
            }

            Tool::Equal(None) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain equal: click 1st line/circle");
            }
            Tool::Equal(Some(_)) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain equal: click 2nd line/circle");
            }

            Tool::Parallel(None) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain parallel: click 1st line");
            }
            Tool::Parallel(Some(_)) => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain parallel: click 2nd line");
            }

            Tool::Angle => {
                response
                    .clone()
                    .on_hover_text_at_pointer("constrain angle: click line");
            }
        }
    }

    fn icon_painter(&self) -> impl FnOnce(egui::Rect, &egui::Painter) {
        match self {
            Tool::Point => point_tool_icon,
            Tool::Line(_) => line_tool_icon,
            Tool::Arc(_) => arc_tool_icon,
            Tool::Circle(_) => circle_tool_icon,
            Tool::Fixed => fixed_tool_icon,
            Tool::Dimension => dim_tool_icon,
            Tool::Horizontal => horizontal_tool_icon,
            Tool::Vertical => vertical_tool_icon,
            Tool::Lerp(_) => lerp_tool_icon,
            Tool::Equal(_) => equal_tool_icon,
            Tool::Parallel(_) => parallel_tool_icon,
            Tool::Angle => angle_tool_icon,
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
            let (l, p, s, d, v, h, i2, e, r, c, n) = ui.input(|i| {
                if i.events.len() == 0 {
                    (
                        false, false, false, false, false, false, false, false, false, false, false,
                    )
                } else {
                    (
                        i.key_pressed(egui::Key::L),
                        i.key_pressed(egui::Key::P),
                        i.key_pressed(egui::Key::S),
                        i.key_pressed(egui::Key::D),
                        i.key_pressed(egui::Key::V),
                        i.key_pressed(egui::Key::H),
                        i.key_pressed(egui::Key::I),
                        i.key_pressed(egui::Key::E),
                        i.key_pressed(egui::Key::R),
                        i.key_pressed(egui::Key::C),
                        i.key_pressed(egui::Key::N),
                    )
                }
            });
            match (l, p, s, d, v, h, i2, e, r, c, n) {
                (true, _, _, _, _, _, _, _, _, _, _) => {
                    self.current = Some(Tool::Line(None));
                    return Some(ToolResponse::Handled);
                }
                (_, true, _, _, _, _, _, _, _, _, _) => {
                    self.current = Some(Tool::Point);
                    return Some(ToolResponse::Handled);
                }
                (_, _, true, _, _, _, _, _, _, _, _) => {
                    self.current = Some(Tool::Fixed);
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, true, _, _, _, _, _, _, _) => {
                    self.current = Some(Tool::Dimension);
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, _, true, _, _, _, _, _, _) => {
                    self.current = Some(Tool::Vertical);
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, _, _, true, _, _, _, _, _) => {
                    self.current = Some(Tool::Horizontal);
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, _, _, _, true, _, _, _, _) => {
                    self.current = Some(Tool::Lerp(None));
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, _, _, _, _, true, _, _, _) => {
                    self.current = Some(Tool::Equal(None));
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, _, _, _, _, _, true, _, _) => {
                    self.current = Some(Tool::Arc(None));
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, _, _, _, _, _, _, true, _) => {
                    self.current = Some(Tool::Circle(None));
                    return Some(ToolResponse::Handled);
                }
                (_, _, _, _, _, _, _, _, _, _, true) => {
                    self.current = Some(Tool::Angle);
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
        drawing: &crate::Data,
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

            let tool_icon_bounds = tool.paint_icon(painter, hp, params, active, i);
            // Show tooltip about tool if hovered
            if let Some(hp) = hp {
                if tool_icon_bounds.contains(hp) {
                    response.clone().on_hover_ui_at_pointer(|ui| {
                        let mut job = egui::text::LayoutJob::default();
                        let font_id = egui::FontId {
                            size: 16.0,
                            family: params.font_id.family.clone(),
                        };
                        job.append(
                            tool.name(),
                            0.0,
                            egui::text::TextFormat {
                                font_id: font_id.clone(),
                                valign: egui::Align::TOP,
                                color: egui::Color32::WHITE,
                                ..Default::default()
                            },
                        );

                        if let Some(key) = tool.key() {
                            job.append(
                                "(",
                                8.0,
                                egui::text::TextFormat {
                                    font_id: font_id.clone(),
                                    color: egui::Color32::WHITE,
                                    valign: egui::Align::TOP,
                                    ..Default::default()
                                },
                            );
                            job.append(
                                key,
                                0.0,
                                egui::text::TextFormat {
                                    font_id: font_id.clone(),
                                    color: egui::Color32::WHITE,
                                    valign: egui::Align::TOP,
                                    underline: egui::Stroke::new(1.0, egui::Color32::WHITE),
                                    ..Default::default()
                                },
                            );
                            job.append(
                                ")",
                                0.0,
                                egui::text::TextFormat {
                                    font_id,
                                    color: egui::Color32::WHITE,
                                    valign: egui::Align::TOP,
                                    ..Default::default()
                                },
                            );
                        }

                        ui.label(job);
                        if let Some(long_tooltip) = tool.long_tooltip() {
                            ui.label(long_tooltip);
                        }
                    });
                }
            }
        }

        if let (Some(hp), Some(tool)) = (hp, self.current.as_ref()) {
            tool.draw_active(painter, response, hp, params, drawing);
        }
    }
}
