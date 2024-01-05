use std::collections::HashMap;
use truck_modeling::*;

fn kurbo_to_truck_vtx(kp: Vec<kurbo::Point>) -> Vec<Vertex> {
    kp.into_iter()
        .map(|p| builder::vertex(Point3::new(p.x, p.y, 0.0)))
        .collect()
}

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

fn face_from_paths(exterior: kurbo::BezPath, cutouts: Vec<kurbo::BezPath>) -> Face {
    let mut verts: HashMap<(u64, u64), Vertex> = HashMap::with_capacity(32);

    let wires = vec![wire_from_path(exterior, &mut verts)];
    let mut face = builder::try_attach_plane(&wires).unwrap();

    for p in cutouts.into_iter() {
        let mut w = wire_from_path(p, &mut verts);
        w.invert();
        face.add_boundary(w);
    }

    face
}

pub fn extrude_from_paths(
    exterior: kurbo::BezPath,
    cutouts: Vec<kurbo::BezPath>,
    height: f64,
) -> Solid {
    let face = face_from_paths(exterior, cutouts);
    builder::tsweep(&face, height * Vector3::unit_z())
}

pub fn extrude_from_points(
    points: Vec<kurbo::Point>,
    boundary_paths_idxs: Vec<Vec<usize>>,
    cutout_paths_idxs: Vec<Vec<usize>>,
    height: f64,
) -> Solid {
    let points = kurbo_to_truck_vtx(points);

    let mut lines = Vec::with_capacity(boundary_paths_idxs.iter().flatten().count());
    for path in boundary_paths_idxs.into_iter() {
        for inds in path.windows(2) {
            lines.push(builder::line(&points[inds[0]], &points[inds[1]]));
        }
    }

    let wire: Wire = lines.into();
    let wires = vec![wire];
    let mut face = builder::try_attach_plane(&wires).unwrap();

    for cutout in cutout_paths_idxs.into_iter() {
        let mut lines = Vec::with_capacity(cutout.len());
        for inds in cutout.windows(2) {
            lines.push(builder::line(&points[inds[0]], &points[inds[1]]));
        }

        let wire: Wire = lines.into();
        face.add_boundary(wire);
    }

    builder::tsweep(&face, height * Vector3::unit_z())
}

pub fn solid_to_stl(s: Solid, tolerance: f64) -> Vec<u8> {
    use truck_meshalgo::tessellation::MeshableShape;
    use truck_meshalgo::tessellation::MeshedShape;

    let mut out = Vec::with_capacity(1024);
    truck_polymesh::stl::write(
        &s.compress().triangulation(tolerance).to_polygon(),
        &mut out,
        truck_polymesh::stl::STLType::Binary,
    )
    .unwrap();

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_convert() {
        assert_eq!(
            kurbo_to_truck_vtx(vec![kurbo::Point::from((1.0, 2.0)),])[0].get_point(),
            Point3::new(1.0, 2.0, 0.0),
        );
    }

    #[test]
    fn extrude_triangle() {
        let _solid = extrude_from_points(
            vec![
                (0.0, 0.0).into(),
                (0.0, 25.).into(),
                (50., 0.0).into(),
                (2.0, 2.0).into(),
                (6.0, 2.).into(),
                (10., 10.).into(),
            ],
            vec![vec![0, 1, 2, 0]],
            vec![vec![3, 4, 5, 3]],
            3.0,
        );

        // use truck_meshalgo::tessellation::MeshableShape;
        // use truck_meshalgo::tessellation::MeshedShape;
        // let mut file = std::fs::File::create("/tmp/ye.stl").unwrap();
        // truck_polymesh::stl::write(
        //     &solid.compress().triangulation(0.02).to_polygon(),
        //     &mut file,
        //     truck_polymesh::stl::STLType::Binary,
        // )
        // .unwrap();
        // let mut file = std::fs::File::create("/tmp/ye.obj").unwrap();
        // truck_polymesh::obj::write(
        //     &solid.compress().triangulation(0.02).to_polygon(),
        //     &mut file,
        // )
        // .unwrap();
    }
}
