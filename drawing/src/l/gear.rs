#[derive(Debug, Clone)]
pub struct SpurGear {
    pub module: f32,
    pub teeth: usize,
    pub pressure_angle: f32,
}

impl Default for SpurGear {
    fn default() -> Self {
        Self {
            module: 1.,
            teeth: 20,
            pressure_angle: 20.,
        }
    }
}

#[allow(dead_code)]
impl SpurGear {
    /// distance from pitch circle to tip circle
    fn addendum(&self) -> f32 {
        self.module
    }
    /// distance from pitch circle to root circle
    fn deddendum(&self) -> f32 {
        1.25 * self.module
    }
    fn clearance(&self) -> f32 {
        self.deddendum() - self.addendum()
    }

    /// pitch circle radius
    pub fn r_pitch(&self) -> f32 {
        self.teeth as f32 * self.module / 2.0
    }
    /// base circle radius
    pub fn r_base(&self) -> f32 {
        self.r_pitch() * (self.pressure_angle * std::f32::consts::PI / 180.).cos()
    }
    /// tip circle radius, aka outer-most radius
    pub fn r_tip(&self) -> f32 {
        self.r_pitch() + self.addendum()
    }
    /// root circle radius
    pub fn r_root(&self) -> f32 {
        self.r_pitch() - self.deddendum()
    }

    // fillet radius
    fn r_fillet(&self) -> f32 {
        1.5 * self.clearance()
    }
    // radius at top of fillet
    fn r_top_fillet(&self) -> f32 {
        let rf = ((self.r_root() + self.r_fillet()).powi(2) - self.r_fillet().powi(2)).sqrt();
        if self.r_base() < rf {
            rf + self.clearance()
        } else {
            rf
        }
    }

    // angular spacing between teeth
    fn pitch_tooth_rads(&self) -> f32 {
        2.0 * std::f32::consts::PI / self.teeth as f32
    }
    // involute angle
    fn base_to_pitch_rads(&self) -> f32 {
        ((self.r_pitch().powi(2) - self.r_base().powi(2)).sqrt() / self.r_base())
            - (self.r_base() / self.r_pitch()).acos()
    }
    fn pitch_to_fillet_rads(&self) -> f32 {
        let bp = self.base_to_pitch_rads();
        if self.r_top_fillet() > self.r_base() {
            bp - ((self.r_top_fillet().powi(2) - self.r_base().powi(2)).sqrt() / self.r_base())
                - (self.r_base() / self.r_top_fillet()).cos()
        } else {
            bp
        }
    }

    fn bez_coeffs(&self) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
        let mut fs: f64 = 0.01; // fraction of length offset from base to avoid singularity
        let rf = self.r_top_fillet();
        let rb = self.r_base();
        if rf > rb {
            fs = (rf.powi(2) - rb.powi(2)) as f64 / (self.r_tip().powi(2) - rb.powi(2)) as f64;
            // offset start to top of fillet
        }

        let fm = fs + (1.0 - fs) / 4.; // fraction of length at junction (25% along profile)
        let mut ded_bez = BezCoeffs::<3>::new(
            self.module as f64,
            self.teeth as f64,
            self.pressure_angle as f64,
            fs,
            fm,
        )
        .involute_bez_coeffs();
        let mut add_bez = BezCoeffs::<3>::new(
            self.module as f64,
            self.teeth as f64,
            self.pressure_angle as f64,
            fm,
            1.,
        )
        .involute_bez_coeffs();

        // Normalize rotation
        let rotate_rads = (-self.base_to_pitch_rads() - self.pitch_tooth_rads() / 4.) as f64;
        for p in [ded_bez.iter_mut(), add_bez.iter_mut()]
            .into_iter()
            .flatten()
        {
            *p = [
                p[0] * rotate_rads.cos() - p[1] * rotate_rads.sin(),
                p[0] * rotate_rads.sin() + p[1] * rotate_rads.cos(),
            ];
        }

        (ded_bez, add_bez)
    }

    pub fn path(&self) -> kurbo::BezPath {
        let (ded_bez, add_bez) = self.bez_coeffs();

        let pt = |px: f64, py: f64, r: f64| {
            kurbo::Point::new(px * r.cos() - py * r.sin(), px * r.sin() + py * r.cos())
        };

        let mut path = kurbo::BezPath::new();
        let r = self.pitch_tooth_rads() as f64;
        let rf = self.r_top_fillet() as f64;
        let rb = self.r_base() as f64;
        let rr = self.r_root() as f64;

        let start_rads = (-self.base_to_pitch_rads() - self.pitch_tooth_rads() / 4.) as f64;
        let fillet = pt(rf, 0., start_rads);
        let fillet_back = pt(rf, 0., -start_rads);

        path.move_to(fillet); // start at top of fillet
        for t in 0..self.teeth {
            let rt = r * t as f64;

            // Line from fillet to base
            if rf < rb {
                path.line_to(pt(ded_bez[0][0], ded_bez[0][1], rt));
            }

            // Climb the tooth
            path.curve_to(
                pt(ded_bez[1][0], ded_bez[1][1], rt),
                pt(ded_bez[2][0], ded_bez[2][1], rt),
                pt(ded_bez[3][0], ded_bez[3][1], rt),
            );
            path.curve_to(
                pt(add_bez[1][0], add_bez[1][1], rt),
                pt(add_bez[2][0], add_bez[2][1], rt),
                pt(add_bez[3][0], add_bez[3][1], rt),
            );

            // TODO: arc?
            path.line_to(pt(add_bez[3][0], -add_bez[3][1], rt));

            // Descend the tooth
            path.curve_to(
                pt(add_bez[2][0], -add_bez[2][1], rt),
                pt(add_bez[1][0], -add_bez[1][1], rt),
                pt(add_bez[0][0], -add_bez[0][1], rt),
            );
            path.curve_to(
                pt(ded_bez[2][0], -ded_bez[2][1], rt),
                pt(ded_bez[1][0], -ded_bez[1][1], rt),
                pt(ded_bez[0][0], -ded_bez[0][1], rt),
            );

            // Line from base to fillet
            if rf < rb {
                path.line_to(pt(fillet_back.x, fillet_back.y, rt));
            }

            // End of fillet
            path.line_to(pt(rr, 0., rt + r / 4.));

            // Arc along root to next fillet
            path.extend(
                kurbo::Arc::new(
                    kurbo::Point::ZERO,
                    kurbo::Vec2::new(rr, rr),
                    rt + r / 4.,
                    r / 2.,
                    0.,
                )
                .append_iter(0.001),
            );
        }

        path.line_to(fillet); // close the path
        path.close_path();
        path
    }
}

use std::f64::consts::PI;

// Adapted to rust code from gearUtils-09.js
// By: Dr A.R.Collins
//
// Original source says: Kindly give credit to Dr A.R.Collins <http://www.arc.id.au/>
// Thanks Doc!!
//
// See: https://www.arc.id.au/GearDrawing.html
struct BezCoeffs<const P: usize> {
    r_base: f64,
    ts: f64,
    te: f64,
}

impl<const P: usize> BezCoeffs<P> {
    // Parameters:
    // module - sets the size of teeth (see gear design texts)
    // numTeeth - number of teeth on the gear
    // pressure angle - angle in degrees, usually 14.5 or 20
    // order - the order of the Bezier curve to be fitted [3, 4, 5, ..]
    // fstart - fraction of distance along tooth profile to start
    // fstop - fraction of distance along profile to stop
    fn new(module: f64, num_teeth: f64, pressure_angle: f64, fstart: f64, fstop: f64) -> Self {
        let r_pitch = module * num_teeth / 2.0; // pitch circle radius
        let phi = pressure_angle; // pressure angle
        let r_base = r_pitch * (phi * PI / 180.0).cos(); // base circle radius
        let r_addendum = r_pitch + module; // addendum radius (outer radius)
        let ta = (r_addendum.powi(2) - r_base.powi(2)).sqrt() / r_base; // involute angle at addendum
        let stop = fstop;

        let start = if fstart < stop { fstart } else { 0.0 };

        let te = (stop.sqrt()) * ta; // involute angle, theta, at end of approx
        let ts = (start.sqrt()) * ta; // involute angle, theta, at start of approx

        BezCoeffs { r_base, ts, te }
    }

    fn cheby_expn_coeffs(&self, j: f64, func: impl Fn(f64) -> f64) -> f64 {
        let n = 50; // a suitably large number  N>>p
        let mut c = 0.0;
        for k in 1..=n {
            c += func((PI * (k as f64 - 0.5) / n as f64).cos())
                * (PI * j * (k as f64 - 0.5) / n as f64).cos();
        }
        2.0 * c / n as f64
    }

    fn cheby_poly_coeffs(&self, p: usize, func: impl Fn(f64) -> f64) -> [f64; 4] {
        let mut coeffs = [0.0, 0.0, 0.0, 0.0];
        let mut fn_coeff = [0.0, 0.0, 0.0, 0.0];
        let mut t = [
            [1.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0],
        ];

        // generate the Chebyshev polynomial coefficient using
        // formula T(k+1) = 2xT(k) - T(k-1) which yields
        for k in 1..=p {
            for j in 0..t[k].len() - 1 {
                t[k + 1][j + 1] = 2.0 * t[k][j];
            }
            for j in 0..t[k - 1].len() {
                t[k + 1][j] -= t[k - 1][j];
            }
        }

        for k in 0..=p {
            fn_coeff[k] = self.cheby_expn_coeffs(k as f64, &func);
            coeffs[k] = 0.0;
        }

        for k in 0..=p {
            for pwr in 0..=p {
                coeffs[pwr] += fn_coeff[k] * t[k][pwr];
            }
        }

        coeffs[0] -= self.cheby_expn_coeffs(0.0, &func) / 2.0; // fix the 0th coeff

        coeffs
    }

    // Equation of involute using the Bezier parameter t as variable
    fn involute_x_bez(&self, t: f64) -> f64 {
        // map t (0 <= t <= 1) onto x (where -1 <= x <= 1)
        let x = t * 2.0 - 1.0;
        // map theta (where ts <= theta <= te) from x (-1 <=x <= 1)
        let theta = x * (self.te - self.ts) / 2.0 + (self.ts + self.te) / 2.0;
        self.r_base * (theta.cos() + theta * theta.sin())
    }

    fn involute_y_bez(&self, t: f64) -> f64 {
        // map t (0 <= t <= 1) onto x (where -1 <= x <= 1)
        let x = t * 2.0 - 1.0;
        // map theta (where ts <= theta <= te) from x (-1 <=x <= 1)
        let theta = x * (self.te - self.ts) / 2.0 + (self.ts + self.te) / 2.0;
        self.r_base * (theta.sin() - theta * theta.cos())
    }

    fn binom(&self, n: usize, k: usize) -> f64 {
        let mut coeff = 1.0;
        for i in n - k + 1..=n {
            coeff *= i as f64;
        }

        for i in 1..=k {
            coeff /= i as f64;
        }

        coeff
    }

    fn bez_coeff(&self, i: usize, func: impl Fn(f64) -> f64) -> f64 {
        // generate the polynomial coeffs in one go
        let poly_coeffs = self.cheby_poly_coeffs(P, &func);

        let mut bc = 0.0;
        for j in 0..=i {
            bc += self.binom(i, j) * poly_coeffs[j] / self.binom(P, j);
        }

        bc
    }

    fn involute_bez_coeffs(&self) -> Vec<[f64; 2]> {
        // calc Bezier coeffs
        let mut bz_coeffs = Vec::with_capacity(P + 1);
        for i in 0..=P {
            let bcoeff = [
                self.bez_coeff(i, |t| self.involute_x_bez(t)),
                self.bez_coeff(i, |t| self.involute_y_bez(t)),
            ];
            bz_coeffs.push(bcoeff);
        }

        bz_coeffs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spur_basic_dimensions() {
        let g = SpurGear {
            module: 2.0,
            teeth: 20,
            ..SpurGear::default()
        };
        assert_eq!(g.r_pitch(), 20.0);
        assert_eq!(g.r_root(), 35.0 / 2.0);
        assert_eq!(g.r_tip(), 44.0 / 2.0);
        assert!((g.r_base() - 37.5877 / 2.0).abs() < 0.01);

        assert_eq!(g.addendum(), 2.0);
        assert_eq!(g.deddendum(), 2.5);
    }

    // #[test]
    // fn spur_debug_paint() {
    //     use tiny_skia::*;
    //     let g = SpurGear {
    //         module: 15.,
    //         teeth: 16,
    //         ..SpurGear::default()
    //     };

    //     let mut cyan = Paint::default();
    //     cyan.set_color_rgba8(50, 127, 150, 255);
    //     cyan.anti_alias = true;
    //     let mut red = Paint::default();
    //     red.set_color_rgba8(250, 27, 50, 255);
    //     red.anti_alias = true;
    //     let mut green = Paint::default();
    //     green.set_color_rgba8(27, 250, 20, 255);
    //     green.anti_alias = true;
    //     let mut blue = Paint::default();
    //     blue.set_color_rgba8(0, 0, 255, 255);
    //     blue.anti_alias = true;

    //     let root = {
    //         let mut pb = PathBuilder::new();
    //         // pb.move_to(100.0, 100.0);
    //         // pb.line_to(150.0, 100.0);
    //         // pb.close();
    //         pb.push_circle(150., 150., g.r_root());
    //         pb.finish().unwrap()
    //     };
    //     let base = {
    //         let mut pb = PathBuilder::new();
    //         pb.push_circle(150., 150., g.r_base());
    //         pb.finish().unwrap()
    //     };
    //     let pitch = {
    //         let mut pb = PathBuilder::new();
    //         pb.push_circle(150., 150., g.r_pitch());
    //         pb.finish().unwrap()
    //     };
    //     let tip = {
    //         let mut pb = PathBuilder::new();
    //         pb.push_circle(150., 150., g.r_tip());
    //         pb.finish().unwrap()
    //     };

    //     let bez = {
    //         let mut pb = PathBuilder::new();
    //         for e in g.path().elements().into_iter() {
    //             use kurbo::PathEl::*;
    //             match e {
    //                 MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
    //                 LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
    //                 CurveTo(p1, p2, p3) => pb.cubic_to(
    //                     p1.x as f32,
    //                     p1.y as f32,
    //                     p2.x as f32,
    //                     p2.y as f32,
    //                     p3.x as f32,
    //                     p3.y as f32,
    //                 ),
    //                 QuadTo(..) => todo!(),
    //                 ClosePath => pb.close(),
    //             }
    //         }

    //         pb.finish().unwrap()
    //     };

    //     let mut stroke = Stroke::default();
    //     stroke.width = 1.0;
    //     let mut pixmap = Pixmap::new(300, 300).unwrap();
    //     pixmap.fill(Color::WHITE);
    //     pixmap.stroke_path(&root, &cyan, &stroke, Transform::identity(), None);
    //     // pixmap.stroke_path(&base, &red, &stroke, Transform::identity(), None);
    //     pixmap.stroke_path(&pitch, &green, &stroke, Transform::identity(), None);
    //     pixmap.stroke_path(&tip, &blue, &stroke, Transform::identity(), None);
    //     pixmap.stroke_path(
    //         &bez,
    //         &red,
    //         &stroke,
    //         Transform::from_translate(150., 150.),
    //         None,
    //     );
    //     pixmap.save_png("/tmp/image.png").unwrap();
    // }
}
