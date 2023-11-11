use egui::Pos2;

#[derive(Debug)]
pub struct LineSegment {
    pub p1: Pos2,
    pub p2: Pos2,
}

impl LineSegment {
    pub fn distance_to_point_sq(&self, point: &Pos2) -> f32 {
        let l2 = self.p1.distance_sq(self.p2);
        if l2 > -f32::EPSILON && l2 < f32::EPSILON {
            // If the line segment is just a point, return the distance between the point and that single point
            return self.p1.distance_sq(*point);
        }

        // Calculate the projection of the point onto the line segment
        let t = ((point.x - self.p1.x) * (self.p2.x - self.p1.x)
            + (point.y - self.p1.y) * (self.p2.y - self.p1.y))
            / l2;

        if t < 0.0 {
            // Closest point is p1
            self.p1.distance_sq(*point)
        } else if t > 1.0 {
            // Closest point is p2
            self.p2.distance_sq(*point)
        } else {
            // Closest point is between p1 and p2
            let projection = Pos2 {
                x: self.p1.x + t * (self.p2.x - self.p1.x),
                y: self.p1.y + t * (self.p2.y - self.p1.y),
            };
            point.distance_sq(projection)
        }
    }

    pub fn intersection_line(&self, other: &LineSegment) -> Option<Pos2> {
        let x1 = self.p1.x;
        let y1 = self.p1.y;
        let x2 = self.p2.x;
        let y2 = self.p2.y;

        let x3 = other.p1.x;
        let y3 = other.p1.y;
        let x4 = other.p2.x;
        let y4 = other.p2.y;

        let denominator = (y4 - y3) * (x2 - x1) - (x4 - x3) * (y2 - y1);

        if denominator == 0.0 {
            // Lines are parallel or coincident
            return None;
        }

        let ua = ((x4 - x3) * (y1 - y3) - (y4 - y3) * (x1 - x3)) / denominator;
        let ub = ((x2 - x1) * (y1 - y3) - (y2 - y1) * (x1 - x3)) / denominator;

        if ua >= 0.0 && ua <= 1.0 && ub >= 0.0 && ub <= 1.0 {
            // Intersection point within the line segments
            let intersection_x = x1 + ua * (x2 - x1);
            let intersection_y = y1 + ua * (y2 - y1);
            Some(Pos2 {
                x: intersection_x,
                y: intersection_y,
            })
        } else {
            // No intersection point within the line segments
            None
        }
    }

    // Find the intersection point between the line segment and the rectangle
    pub fn intersection_rect(&self, rect: &egui::Rect) -> Option<Pos2> {
        let egui::Rect { min, max } = rect;

        // Calculate the intersection points with the rectangle's four sides
        let intersections = [
            self.intersection_line(&LineSegment {
                p1: Pos2 { x: min.x, y: max.y },
                p2: Pos2 { x: max.x, y: max.y },
            }),
            self.intersection_line(&LineSegment {
                p1: Pos2 { x: max.x, y: max.y },
                p2: Pos2 { x: max.x, y: min.y },
            }),
            self.intersection_line(&LineSegment {
                p1: Pos2 { x: min.x, y: min.y },
                p2: Pos2 { x: max.x, y: min.y },
            }),
            self.intersection_line(&LineSegment {
                p1: Pos2 { x: min.x, y: max.y },
                p2: Pos2 { x: min.x, y: min.y },
            }),
        ];

        // Filter out None values (no intersection) and select the closest intersection point
        let closest_intersection =
            intersections
                .into_iter()
                .flatten()
                .fold(None, |closest, current| match closest {
                    None => Some(current),
                    Some(existing) => {
                        let current_distance = self.p1.distance(current);
                        let existing_distance = self.p1.distance(existing);
                        if current_distance < existing_distance {
                            Some(current)
                        } else {
                            Some(existing)
                        }
                    }
                });

        closest_intersection
    }
}
