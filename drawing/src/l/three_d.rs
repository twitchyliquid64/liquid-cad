use crate::data::CADOp;
use std::collections::HashMap;
use truck_modeling::*;

fn wire_from_path(path: kurbo::BezPath, verts: &mut HashMap<(u64, u64), Vertex>) -> Wire {
    let mut vert = |p: kurbo::Point| {
        let (x, y) = (p.x, p.y);

        let k = (x.to_bits(), y.to_bits());
        if let Some(v) = verts.get(&k) {
            v.clone()
        } else {
            let v = builder::vertex(Point3::new(x, y, 0.0));
            verts.insert(k, v.clone());
            v
        }
    };

    let mut edges = Vec::with_capacity(path.elements().len());
    let mut last: Option<Vertex> = None;
    for seg in path.segments() {
        match seg {
            kurbo::PathSeg::Line(kurbo::Line { p0, p1 }) => {
                let end = vert(p1);
                edges.push(builder::line(&last.unwrap_or(vert(p0)), &end));
                last = Some(end);
            }
            kurbo::PathSeg::Quad(kurbo::QuadBez { p0, p1, p2 }) => {
                let end = vert(p2);
                edges.push(builder::bezier(
                    &last.unwrap_or(vert(p0)),
                    &end,
                    vec![Point3::new(p1.x, p1.y, 0.0)],
                ));
                last = Some(end);
            }
            kurbo::PathSeg::Cubic(kurbo::CubicBez { p0, p1, p2, p3 }) => {
                let end = vert(p3);
                edges.push(builder::bezier(
                    &last.unwrap_or(vert(p0)),
                    &end,
                    vec![Point3::new(p1.x, p1.y, 0.0), Point3::new(p2.x, p2.y, 0.0)],
                ));
                last = Some(end);
            }
        }
    }

    let (start, end) = (edges[0].front(), edges.last().unwrap().back());
    if start != end {
        edges.push(builder::line(end, start));
    }

    edges.into()
}

fn op_parents(ops: &Vec<(CADOp, kurbo::BezPath)>) -> Vec<isize> {
    // Using the bounding box of each shape, compute which operation it is dependent upon.
    use kurbo::Shape;
    let op_bb: Vec<_> = ops.iter().map(|(_, p)| p.bounding_box()).collect();
    ops.iter()
        .enumerate()
        .map(|(i, _)| {
            let mut best: (isize, f64) = (-1, f64::INFINITY);
            for i2 in 0..ops.len() {
                if i == i2 {
                    continue;
                }
                if op_bb[i2].area() > op_bb[i].area()
                    && op_bb[i2].intersect(op_bb[i]) == op_bb[i]
                    && op_bb[i2].area() < best.1
                {
                    best = (i2 as isize, op_bb[i2].area());
                }
            }
            best.0
        })
        .collect()
}

pub fn extrude_from_paths(
    exterior: kurbo::BezPath,
    ops: Vec<(CADOp, kurbo::BezPath)>,
    height: f64,
) -> Solid {
    use kurbo::Shape;
    let mut verts: HashMap<(u64, u64), Vertex> = HashMap::with_capacity(32);
    // let op_parent_idx = op_parents(&ops);

    let ea = exterior.area();
    let base_face: Face =
        builder::try_attach_plane(&vec![wire_from_path(exterior, &mut verts)]).unwrap();
    let mut base: Shell = builder::tsweep(&base_face, height * Vector3::unit_z())
        .into_boundaries()
        .pop()
        .unwrap();
    let (bottom_idx, top_idx) = (0, base.len() - 1);

    for (op, path) in ops.into_iter() {
        let pa = path.area();
        if !matches!(op, CADOp::Hole) {
            panic!("unexpected op! {:?}", op);
        }

        let mut w = wire_from_path(path, &mut verts);
        if ea.signum() == pa.signum() {
            w.invert(); // HACK: truck cares about winding order
        }
        let shell = builder::tsweep(&w, height * Vector3::unit_z());
        let b = shell.extract_boundaries();

        // Extract copies of the wires representing the boundaries of the hole.
        // Use these to insert holes in the boundary of the base shell.
        let bottom_wire = b.first().unwrap();
        base[bottom_idx].add_boundary(bottom_wire.inverse());
        let top_wire = b.last().unwrap();
        base[top_idx].add_boundary(top_wire.inverse());

        base.extend(shell);
    }

    Solid::new(vec![base])
}

pub fn solid_to_stl(s: Solid, tolerance: f64) -> Vec<u8> {
    use truck_meshalgo::tessellation::MeshableShape;
    use truck_meshalgo::tessellation::MeshedShape;
    let mut mesh = s.triangulation(tolerance).to_polygon();

    use truck_meshalgo::filters::OptimizingFilter;
    mesh.put_together_same_attrs()
        .remove_degenerate_faces()
        .remove_unused_attrs();

    let mut out = Vec::with_capacity(1024);
    truck_polymesh::stl::write(&mesh, &mut out, truck_polymesh::stl::STLType::Binary).unwrap();

    out
}

pub fn solid_to_obj(s: Solid, tolerance: f64) -> Vec<u8> {
    use truck_meshalgo::tessellation::MeshableShape;
    use truck_meshalgo::tessellation::MeshedShape;
    let mut mesh = s.triangulation(tolerance).to_polygon();

    use truck_meshalgo::filters::OptimizingFilter;
    mesh.put_together_same_attrs()
        .remove_degenerate_faces()
        .remove_unused_attrs();

    let mut out = Vec::with_capacity(1024);
    truck_polymesh::obj::write(&mesh, &mut out).unwrap();

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_parents_basic() {
        use kurbo::Shape;

        // Empty
        assert_eq!(op_parents(&vec![]), vec![],);

        // Not nested, 1 element
        assert_eq!(
            op_parents(&vec![(
                CADOp::Hole,
                kurbo::Rect {
                    x0: 1.0,
                    y0: 1.0,
                    x1: 5.0,
                    y1: 5.0
                }
                .into_path(0.1)
            ),]),
            vec![-1],
        );
        // Not nested, 2 elements
        assert_eq!(
            op_parents(&vec![
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 1.0,
                        y0: 1.0,
                        x1: 5.0,
                        y1: 5.0
                    }
                    .into_path(0.1)
                ),
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 6.0,
                        y0: 6.0,
                        x1: 9.0,
                        y1: 9.0
                    }
                    .into_path(0.1)
                ),
            ]),
            vec![-1, -1],
        );
        // Intersecting
        assert_eq!(
            op_parents(&vec![
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 1.0,
                        y0: 1.0,
                        x1: 5.0,
                        y1: 5.0
                    }
                    .into_path(0.1)
                ),
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 4.0,
                        y0: 4.0,
                        x1: 6.0,
                        y1: 6.0
                    }
                    .into_path(0.1)
                ),
            ]),
            vec![-1, -1],
        );

        // Basic
        assert_eq!(
            op_parents(&vec![
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 1.0,
                        y0: 1.0,
                        x1: 5.0,
                        y1: 5.0
                    }
                    .into_path(0.1)
                ),
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 2.0,
                        y0: 2.0,
                        x1: 3.0,
                        y1: 3.0
                    }
                    .into_path(0.1)
                ),
            ]),
            vec![-1, 0],
        );

        // Multiple
        assert_eq!(
            op_parents(&vec![
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 1.0,
                        y0: 1.0,
                        x1: 5.0,
                        y1: 5.0
                    }
                    .into_path(0.1)
                ),
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 2.5,
                        y0: 2.1,
                        x1: 2.9,
                        y1: 2.8
                    }
                    .into_path(0.1)
                ),
                (
                    CADOp::Hole,
                    kurbo::Rect {
                        x0: 2.0,
                        y0: 2.0,
                        x1: 3.0,
                        y1: 3.0
                    }
                    .into_path(0.1)
                ),
            ]),
            vec![-1, 2, 0],
        );
    }
}
