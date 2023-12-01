extern crate nalgebra as na;
use super::*;
use na::{DMatrix, DVector, Dyn, OMatrix, OVector};
use num::ToPrimitive;
use std::collections::HashMap;

pub fn sigmoid(v: f64) -> f64 {
    1.0 / (1.0 + f64::exp(-v))
}

#[derive(Default, Clone, Debug)]
pub struct DumbassSolverState {
    resolved: HashMap<Variable, Concrete>,

    vars: Vec<Variable>,
    residuals: Vec<Expression>,
    jacobians: Vec<Vec<Expression>>,
}

impl DumbassSolverState {
    pub fn new(
        concrete: HashMap<Variable, Concrete>,
        solve_for: Vec<Variable>,
        residuals: Vec<Expression>,
    ) -> Self {
        let jacobians = residuals
            .iter()
            .map(|fx| {
                solve_for
                    .iter()
                    .map(|var| fx.derivative_wrt(&var))
                    .collect()
            })
            .collect();

        Self {
            resolved: concrete,
            vars: solve_for,
            residuals,
            jacobians,
        }
    }
}

struct VarResolver<'a> {
    resolved: &'a HashMap<Variable, Concrete>,
    vars: &'a Vec<Variable>,

    x: &'a OVector<f64, Dyn>,
}

impl<'a> Resolver for VarResolver<'a> {
    fn resolve_variable(&mut self, v: &Variable) -> Result<Concrete, ResolveErr> {
        match self.resolved.get(v) {
            Some(c) => {
                return Ok(c.clone());
            }
            None => {}
        };

        for (i, v2) in self.vars.iter().enumerate() {
            if v == v2 {
                return Ok(Concrete::Float(self.x[i]));
            }
        }

        Err(ResolveErr::UnknownVar(v.clone()))
    }
}

/// Iterative gradient-descent newton-method-vibes solver.
///
/// My math understanding is trash.
///
/// Basically, the way it works is that it tries some values and runs
/// the residual functions to see how trash they are (they should return
/// zero if the constraint holds). We then update the values based on how
/// wrong the residuals were: in the right direction and the right amount.
///
/// Whats the right direction and right amount? thats based on the jacobian
/// of the residual function with respect to each variable we are trying to solve for.
/// Don't be scared by the 'jacobian' term, it basically just means the derivative
/// with respect to a variable. So basically it represents the 'slope' of how much
/// the correctness of the function contributes to some variable, so thats where
/// we get our 'right direction' and 'right amount' information.
///
/// For instance, lets say our residual function is: 5 - x (AKA the
/// correct solution for x is 5). The derivative with respect to
/// x is just: -x. Thats all that means.
///
/// Anyway, my intent in calling this a dumbass solver was to
/// try and get across that I have no idea what I'm doing, and
/// i get super lost whenever i try and read a math paper about
/// constraint solvers. But, I think I've picked up just enough
/// to get something over the line, and the result is this.
///
/// Where possible, I will always tradeoff being understandable
/// and practical over being clever and efficient.
///
/// I have a few random multipliers in here, that I thought would
/// help convergence at the time. Idk if they actually do.
#[derive(Clone, Debug)]
pub struct DumbassSolver {
    iteration: usize,

    // guess of each variable
    x: OVector<f64, Dyn>,
    // residual calculation result
    fx: OVector<f64, Dyn>,
    // jacobian by [residual, variable]
    j: OMatrix<f64, Dyn, Dyn>,
}

impl DumbassSolver {
    const MAX_ITER: usize = 450;
    const STEP_MUL: f64 = 1.0;

    const AVG_FX_TOLERANCE: f64 = 0.0008;

    pub fn new(st: &DumbassSolverState) -> Self {
        let iteration = 0;

        Self {
            iteration,
            x: DVector::from_element(st.vars.len(), -8.001),
            fx: DVector::from_element(st.residuals.len(), 0.0),
            j: DMatrix::from_element(st.residuals.len(), st.vars.len(), 0.0),
        }
    }

    pub fn new_with_initials(st: &DumbassSolverState, initials: Vec<f64>) -> Self {
        let iteration = 0;

        assert!(st.vars.len() == initials.len());
        Self {
            iteration,
            x: DVector::from(initials),
            fx: DVector::from_element(st.residuals.len(), 0.0),
            j: DMatrix::from_element(st.residuals.len(), st.vars.len(), 0.0),
        }
    }

    fn solve_step(&mut self, st: &mut DumbassSolverState) {
        let DumbassSolver { x, fx, j, .. } = self;

        let mut resolver = VarResolver {
            x: &x,
            vars: &st.vars,
            resolved: &st.resolved,
        };

        // Compute jacobian
        for (row, jacs) in st.jacobians.iter().enumerate() {
            for (col, j_fn) in jacs.iter().enumerate() {
                let v = match j_fn.evaluate(&mut resolver, 0).unwrap() {
                    Concrete::Float(f) => f as f64,
                    Concrete::Rational(r) => r.to_f64().unwrap(),
                };
                // if v.is_nan() {
                //     v = 0.0;
                // }
                j[(row, col)] = v;
            }
        }

        // Softmax the jacobian for each variable, multiplied by
        // the proportion of variables which are non-zero
        let total_terms = st.vars.len() as f64;
        for mut col in j.column_iter_mut() {
            let exp_sum = col.iter().fold(0.0, |acc, x| acc + x.exp());
            let terms_non_zero = col
                .iter()
                .map(|j| *j == 0.0)
                .fold(0.0, |acc, zero| acc + if zero { 0.0 } else { 1.0 });
            for j in col.iter_mut() {
                *j *= j.exp() / exp_sum * terms_non_zero / total_terms;
            }
        }

        // Compute residuals
        for (row, exp) in st.residuals.iter().enumerate() {
            let res = match exp.evaluate(&mut resolver, 0).unwrap() {
                Concrete::Float(f) => f as f64,
                Concrete::Rational(r) => r.to_f64().unwrap(),
            };
            fx[row] = res;
        }

        // println!(
        //     "mul={}\nx:{}j:{}fx:{}",
        //     DumbassSolver::apply_multiplier(self.iteration),
        //     x,
        //     j,
        //     fx
        // );

        // Update guesses
        let adjustment = (fx.transpose() * &*j).transpose() * -DumbassSolver::STEP_MUL;
        *x += adjustment;
    }

    pub fn solve(
        &mut self,
        st: &mut DumbassSolverState,
    ) -> Result<Vec<(Variable, f64)>, (f64, Vec<(Variable, f64)>)> {
        let mut total_fx = 999999.0;
        while self.iteration < DumbassSolver::MAX_ITER {
            self.solve_step(st);

            total_fx = self.fx.iter().fold(0.0, |acc, x| acc + x.abs());
            if (total_fx.abs() / st.vars.len() as f64) < DumbassSolver::AVG_FX_TOLERANCE {
                break;
            }
            self.iteration += 1;
        }

        let results = st
            .vars
            .iter()
            .enumerate()
            .map(|(i, v)| (v.clone(), self.x[i]))
            .collect();
        if self.iteration < DumbassSolver::MAX_ITER {
            Ok(results)
        } else {
            Err((total_fx, results))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state() {
        let state = DumbassSolverState::new(
            HashMap::from([
                ("x0".into(), Concrete::Float(0.0)),
                ("y0".into(), Concrete::Float(0.0)),
            ]),
            vec!["x1".into(), "y1".into()],
            vec![Expression::parse("5 - sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap()],
        );

        // Make sure jacobians were computed correctly
        // I know this looks wonky but wolframalpha computed the same
        assert_eq!(
            state.jacobians,
            vec![vec![
                Expression::parse(
                    "-((2 * (x1 - x0)) / (2 * sqrt((((x1 - x0))^2 + ((y1 - y0))^2))))",
                    false
                )
                .unwrap(),
                Expression::parse(
                    "-((2 * (y1 - y0)) / (2 * sqrt((((x1 - x0))^2 + ((y1 - y0))^2))))",
                    false
                )
                .unwrap(),
            ],],
        );

        let _ = DumbassSolver::new(&state);
    }

    #[test]
    fn basic() {
        let mut state = DumbassSolverState::new(
            HashMap::from([
                ("x0".into(), Concrete::Float(0.0)),
                ("y0".into(), Concrete::Float(0.0)),
            ]),
            vec!["x1".into(), "y1".into()],
            vec![Expression::parse("5 - sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap()],
        );
        let mut solver = DumbassSolver::new(&state);

        // Set some initial conditions.
        solver.x[0] = 0.001;
        solver.x[1] = 1.000;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration < 15);
        assert!(ret[0].1 < 0.1);
        let f = (ret[0].1 + ret[1].1).abs();
        assert!(f > 4.9 && f < 5.1); // trashy check but gets the point across.

        // Different initial conditions solve towards a proportionally-biased solution.
        solver = DumbassSolver::new(&state);
        solver.x[0] = 1.0;
        solver.x[1] = 1.0;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration < 15);
        // trashy check but gets the point across.
        assert!(ret[0].1 > 3.4 && ret[0].1 < 3.6);
        assert!(ret[1].1 > 3.4 && ret[1].1 < 3.6);
    }

    #[test]
    fn two_dist_intersection() {
        let mut state = DumbassSolverState::new(
            HashMap::from([
                ("d".into(), Concrete::Float(5.0)),
                ("x0".into(), Concrete::Float(0.0)),
                ("y0".into(), Concrete::Float(0.0)),
                ("x2".into(), Concrete::Float(2.5)),
                ("y2".into(), Concrete::Float(1.0)),
            ]),
            vec!["x1".into(), "y1".into()],
            vec![
                Expression::parse("d - sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap(),
                Expression::parse("d - sqrt((x1-x2)^2 + (y1-y2)^2)", false).unwrap(),
            ],
        );
        let mut solver = DumbassSolver::new(&state);

        solver.x[0] = 1.000;
        solver.x[1] = 3.000;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration < 80);
        assert!(ret[0].1 < 0.0001);
        assert!(solver.x[0] < 0.1);
    }

    #[test]
    fn dist_snake() {
        // (0,0) ---88--- (x1,y1) ---88--- (x2,y2)
        //
        // For two points from the origin constrained by distances, we observed
        // that it fails to solve when initial x > 0. This test makes sure
        // we do.
        let mut state = DumbassSolverState::new(
            HashMap::from([
                ("x0".into(), Concrete::Float(0.0)),
                ("y0".into(), Concrete::Float(0.0)),
            ]),
            vec!["x1".into(), "y1".into(), "x2".into(), "y2".into()],
            vec![
                Expression::parse("88 - sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap(),
                Expression::parse("88 - sqrt((x2-x1)^2 + (y2-y1)^2)", false).unwrap(),
            ],
        );
        let mut solver = DumbassSolver::new(&state);

        // Make sure we can solve in the good quadrant
        solver.x[0] = -62.0;
        solver.x[1] = -62.0;
        solver.x[0] = -3000.0;
        solver.x[1] = -3000.0;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration < 120);
        let dist_leg_1 = (ret[0].1.powi(2) + ret[1].1.powi(2)).sqrt();
        assert!(dist_leg_1 > 87.9 && dist_leg_1 < 88.1);

        // Now test we can solve it in the bad quadrant
        solver = DumbassSolver::new(&state);
        solver.x[0] = 62.0;
        solver.x[1] = 62.0;
        solver.x[0] = 800.0;
        solver.x[1] = 800.0;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration < 160);
        let dist_leg_1 = (ret[0].1.powi(2) + ret[1].1.powi(2)).sqrt();
        assert!(dist_leg_1 > 87.9 && dist_leg_1 < 88.1);
    }
}
