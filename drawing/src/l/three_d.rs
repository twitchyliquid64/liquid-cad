use truck_modeling::*;

fn kurbo_to_truck_vtx(kp: Vec<kurbo::Point>) -> Vec<Vertex> {
    kp.into_iter()
        .map(|p| builder::vertex(Point3::new(p.x, p.y, 0.0)))
        .collect()
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

pub fn solid_to_stl(s: Solid) -> Vec<u8> {
    use truck_meshalgo::tessellation::MeshableShape;
    use truck_meshalgo::tessellation::MeshedShape;

    let mut out = Vec::with_capacity(1024);
    truck_polymesh::stl::write(
        &s.compress().triangulation(0.02).to_polygon(),
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
