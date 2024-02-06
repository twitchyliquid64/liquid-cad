use crate::data::CADOp;
use std::collections::{HashMap, HashSet};
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
    let op_parent_idx = op_parents(&ops);

    let ea = exterior.area();
    let mut base_wire = wire_from_path(exterior, &mut verts);
    if ea.signum() < 0.0 {
        base_wire.invert();
    }
    let base_face: Face = builder::try_attach_plane(&vec![base_wire]).unwrap();
    let mut base: Shell = builder::tsweep(&base_face, height * Vector3::unit_z())
        .into_boundaries()
        .pop()
        .unwrap();
    // base.face_iter_mut().for_each(|f| if !f.orientation(){ f.invert(); });
    let (bottom_idx, top_idx) = (0, base.len() - 1);

    // done_parents tracks the index into the base shell for the top and bottom
    // faces which represent the geometry created at that ops index.
    let mut done_parents: HashMap<isize, (usize, usize)> = HashMap::with_capacity(ops.len());
    done_parents.insert(-1, (bottom_idx, top_idx));

    let mut done: HashSet<usize> = HashSet::with_capacity(ops.len());
    while done.len() < ops.len() {
        let next_idx = ops
            .iter()
            .enumerate()
            .filter(|(i, _)| !done.contains(i) && done_parents.contains_key(&op_parent_idx[*i]))
            .map(|(i, _)| i)
            .find(|_| true);

        match next_idx {
            None => unreachable!(),
            Some(i) => {
                let (op, path) = &ops[i];
                // println!("i={}: ({}) {:?}, {:?}", i, op_parent_idx[i], op, path);
                let mut w = wire_from_path(path.clone(), &mut verts);

                let (bottom_idx, top_idx) = *done_parents.get(&op_parent_idx[i]).unwrap();
                let bottom_offset_z = base[bottom_idx]
                    .absolute_boundaries()
                    .iter()
                    .map(|e| e.vertex_iter())
                    .flatten()
                    .fold(-99999.0, |acc: f64, v| acc.max(v.get_point().z));
                let top_offset_z = base[top_idx]
                    .absolute_boundaries()
                    .iter()
                    .map(|e| e.vertex_iter())
                    .flatten()
                    .fold(-99999.0, |acc: f64, v| acc.max(v.get_point().z));

                match op {
                    CADOp::Hole => {
                        if path.area().signum() > 0.0 {
                            w.invert(); // negative geometry must have edges clockwise
                        }
                        let f: Face = builder::try_attach_plane(&vec![w]).unwrap();
                        let tf = builder::tsweep(
                            &f,
                            (top_offset_z - bottom_offset_z) * Vector3::unit_z(),
                        );
                        let solid = builder::translated(&tf, bottom_offset_z * Vector3::unit_z());
                        let mut b = solid.into_boundaries().pop().unwrap();

                        // Extract copies of the wires representing the boundaries of the hole.
                        // Use these to insert holes in the boundary of the base shell.
                        let bottom_wire = &b.first().unwrap().boundaries()[0];
                        base[bottom_idx].add_boundary(bottom_wire.clone());
                        let top_wire = &b.last().unwrap().boundaries()[0];
                        base[top_idx].add_boundary(top_wire.clone());

                        b.pop();
                        base.extend(b.into_iter().skip(1));
                    }
                    CADOp::Extrude(amt, false) => {
                        // on top
                        if path.area().signum() < 0.0 {
                            w.invert(); // regular geometry must have edges counter-clockwise
                        }
                        let f: Face = builder::try_attach_plane(&vec![w]).unwrap();
                        let tf = builder::tsweep(&f, *amt * Vector3::unit_z());
                        let solid = builder::translated(&tf, top_offset_z * Vector3::unit_z());
                        let b = solid.into_boundaries().pop().unwrap();

                        // Cut the base shape at the boundary so we can glue the extrusion
                        let bottom_wire = &b.first().unwrap().boundaries()[0];
                        base[top_idx].add_boundary(bottom_wire.clone());

                        // Add the faces of the extrusion except the bottom
                        base.extend(b.into_iter().skip(1));

                        done_parents.insert(i as isize, (bottom_idx, base.len() - 1));
                    }
                    CADOp::Extrude(amt, true) => {
                        // on bottom
                        if path.area().signum() < 0.0 {
                            w.invert(); // regular geometry must have edges counter-clockwise
                        }
                        let f: Face = builder::try_attach_plane(&vec![w]).unwrap();
                        let tf = builder::tsweep(&f, *amt * Vector3::unit_z());
                        let solid =
                            builder::translated(&tf, (bottom_offset_z - *amt) * Vector3::unit_z());
                        let mut b = solid.into_boundaries().pop().unwrap();

                        // Cut the base shape at the boundary so we can glue the extrusion
                        let top_wire = &b.last().unwrap().boundaries()[0];
                        base[bottom_idx].add_boundary(top_wire.clone());

                        let next_face_idx = base.len();

                        // Add the faces of the extrusion except the top
                        b.pop();
                        base.extend(b.into_iter());

                        done_parents.insert(i as isize, (next_face_idx, top_idx));
                    }
                    CADOp::Bore(amt, false) => {
                        // on top
                        if path.area().signum() > 0.0 {
                            w.invert(); // negative geometry must have edges clockwise
                        }
                        let f: Face = builder::try_attach_plane(&vec![w]).unwrap();
                        let tf = builder::tsweep(&f, *amt * Vector3::unit_z());
                        let solid =
                            builder::translated(&tf, (top_offset_z - *amt) * Vector3::unit_z());
                        let mut b = solid.into_boundaries().pop().unwrap();

                        let top_wire = &b.last().unwrap().boundaries()[0];
                        base[top_idx].add_boundary(top_wire.clone());

                        let next_face_idx = base.len(); // next face will be the bottom of the bore

                        // Add the faces of the bore except the top
                        b.pop();
                        base.extend(b.into_iter());
                        done_parents.insert(i as isize, (bottom_idx, next_face_idx));
                    }
                    CADOp::Bore(amt, true) => {
                        // on bottom
                        if path.area().signum() > 0.0 {
                            w.invert(); // negative geometry must have edges clockwise
                        }
                        let f: Face = builder::try_attach_plane(&vec![w]).unwrap();
                        let tf = builder::tsweep(&f, *amt * Vector3::unit_z());
                        let solid = builder::translated(&tf, bottom_offset_z * Vector3::unit_z());
                        let b = solid.into_boundaries().pop().unwrap();

                        let bottom_wire = &b.first().unwrap().boundaries()[0];
                        base[bottom_idx].add_boundary(bottom_wire.clone());

                        // Add the faces of the bore except the bottom
                        base.extend(b.into_iter().skip(1));
                        done_parents.insert(i as isize, (base.len() - 1, top_idx));
                    }
                }
                done.insert(i);
            }
        }
    }

    // use truck_topology::{
    //     EdgeDisplayFormat, FaceDisplayFormat, ShellDisplayFormat, VertexDisplayFormat,
    //     WireDisplayFormat,
    // };
    // let vertex_format = VertexDisplayFormat::AsPoint;
    // let edge_format = EdgeDisplayFormat::VerticesTuple { vertex_format };
    // let wire_format = WireDisplayFormat::EdgesList { edge_format };
    // let face_format = FaceDisplayFormat::Full { wire_format };
    // println!(
    //     "{:#?}",
    //     base.display(ShellDisplayFormat::FacesListTuple { face_format })
    // );

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
    use truck_meshalgo::analyzers::WithPointCloud;
    use truck_meshalgo::tessellation::MeshableShape;
    use truck_meshalgo::tessellation::MeshedShape;

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

    #[test]
    fn extrude_basic() {
        use kurbo::Shape;

        let rect = extrude_from_paths(
            kurbo::Rect {
                x0: 1.0,
                y0: 1.0,
                x1: 5.0,
                y1: 5.0,
            }
            .into_path(0.1),
            vec![],
            4.0,
        );
        for (i, (f, want)) in rect
            .face_iter()
            .zip(&[false, true, true, true, true, true])
            .enumerate()
        {
            assert_eq!(
                f.orientation(),
                *want,
                "face {} was inverted:\n{}",
                i,
                f.vertex_iter()
                    .map(|v| format!(" - {:?}", v).to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        let mesh = rect.triangulation(0.1).to_polygon();
        assert_eq!(
            true,
            mesh.collide_with_neiborhood_of(
                &[Point3 {
                    x: 1.5,
                    y: 1.5,
                    z: 1.5
                }],
                3.
            )
        );

        let holey_rect = extrude_from_paths(
            kurbo::Rect {
                x0: 1.0,
                y0: 1.0,
                x1: 5.0,
                y1: 5.0,
            }
            .into_path(0.1),
            vec![(
                CADOp::Hole,
                kurbo::Rect {
                    x0: 2.0,
                    y0: 2.0,
                    x1: 4.0,
                    y1: 4.0,
                }
                .into_path(0.1),
            )],
            4.0,
        );
        let mesh = holey_rect.triangulation(0.1).to_polygon();
        assert_eq!(
            false,
            mesh.collide_with_neiborhood_of(
                &[Point3 {
                    x: 3.0,
                    y: 3.0,
                    z: 2.0,
                }],
                1.0
            )
        );
    }

    #[test]
    fn extrude_hole() {
        use kurbo::Shape;

        let holey_rect = extrude_from_paths(
            kurbo::Rect {
                x0: 1.0,
                y0: 1.0,
                x1: 5.0,
                y1: 5.0,
            }
            .into_path(0.1),
            vec![(
                CADOp::Hole,
                kurbo::Rect {
                    x0: 2.0,
                    y0: 2.0,
                    x1: 4.0,
                    y1: 4.0,
                }
                .into_path(0.1),
            )],
            4.0,
        );
        let mesh = holey_rect.triangulation(0.1).to_polygon();
        assert_eq!(
            false,
            mesh.collide_with_neiborhood_of(
                &[Point3 {
                    x: 3.0,
                    y: 3.0,
                    z: 2.0,
                }],
                1.0
            )
        );
    }

    #[test]
    fn extrude_extrusion() {
        use kurbo::Shape;

        let holey_rect = extrude_from_paths(
            kurbo::Rect {
                x0: 1.0,
                y0: 1.0,
                x1: 5.0,
                y1: 5.0,
            }
            .into_path(0.1),
            vec![(
                CADOp::Extrude(4.5, false),
                kurbo::Rect {
                    x0: 2.0,
                    y0: 2.0,
                    x1: 4.0,
                    y1: 4.0,
                }
                .into_path(0.1),
            )],
            4.0,
        );
        let mesh = holey_rect.triangulation(0.1).to_polygon();
        assert_eq!(
            true,
            mesh.collide_with_neiborhood_of(
                &[Point3 {
                    x: 3.0,
                    y: 3.0,
                    z: 8.0,
                }],
                1.0
            )
        );
    }

    #[test]
    fn extrude_bore() {
        use kurbo::Shape;

        let holey_rect = extrude_from_paths(
            kurbo::Rect {
                x0: 1.0,
                y0: 1.0,
                x1: 5.0,
                y1: 5.0,
            }
            .into_path(0.1),
            vec![(
                CADOp::Bore(5.0, false),
                kurbo::Rect {
                    x0: 2.0,
                    y0: 2.0,
                    x1: 4.0,
                    y1: 4.0,
                }
                .into_path(0.1),
            )],
            10.0,
        );
        let mesh = holey_rect.triangulation(0.1).to_polygon();
        assert_eq!(
            false,
            mesh.collide_with_neiborhood_of(
                &[Point3 {
                    x: 3.0,
                    y: 3.0,
                    z: 10.0,
                }],
                1.0
            )
        );
    }
}
