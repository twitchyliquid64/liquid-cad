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
#[derive(Clone, Debug)]
pub struct DumbassSolver {
    iteration: usize,

    // guess of each variable
    x: OVector<f64, Dyn>,
    // residual calculation result
    fx: OVector<f64, Dyn>,
    // jacobian by [variable, residual]
    j: OMatrix<f64, Dyn, Dyn>,
}

impl DumbassSolver {
    const MAX_ITER: usize = 350;
    const DELTA_MUL: f64 = 0.95;

    const TOTAL_FX_TOLERANCE: f64 = 0.0005;

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

    fn apply_multiplier(iteration: usize) -> f64 {
        // in WRA: plot 0.95 - sigmoid(x/18)/8, x=0..30
        return DumbassSolver::DELTA_MUL - sigmoid(iteration as f64 / 18.0) / 8.0;
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
                j[(row, col)] = match j_fn.evaluate(&mut resolver, 0).unwrap() {
                    Concrete::Float(f) => f as f64,
                    Concrete::Rational(r) => r.to_f64().unwrap(),
                };
            }
        }

        // Compute residuals
        for (row, exp) in st.residuals.iter().enumerate() {
            fx[row] = match exp.evaluate(&mut resolver, 0).unwrap() {
                Concrete::Float(f) => f as f64,
                Concrete::Rational(r) => r.to_f64().unwrap(),
            };
        }

        // Update guesses
        let adjustment =
            (fx.transpose() * &*j).transpose() * -DumbassSolver::apply_multiplier(self.iteration);
        *x += adjustment;

        // println!("j: {}fx: {}x: {}", j, fx, x);
    }

    pub fn solve(&mut self, st: &mut DumbassSolverState) -> Result<Vec<(Variable, f64)>, ()> {
        while self.iteration < DumbassSolver::MAX_ITER {
            self.solve_step(st);

            let total_fx = self.fx.iter().fold(0.0, |acc, x| acc + x.abs());
            if total_fx.abs() < DumbassSolver::TOTAL_FX_TOLERANCE {
                break;
            }
            self.iteration += 1;
        }

        if self.iteration < DumbassSolver::MAX_ITER {
            Ok(st
                .vars
                .iter()
                .enumerate()
                .map(|(i, v)| (v.clone(), self.x[i]))
                .collect())
        } else {
            Err(())
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

        assert!(solver.iteration < 10);
        assert!(ret[0].1 < 0.1);
        let f = (ret[0].1 + ret[1].1).abs();
        assert!(f > 4.9 && f < 5.1); // trashy check but gets the point across.

        // Different initial conditions solve towards a proportionally-biased solution.
        solver = DumbassSolver::new(&state);
        solver.x[0] = 1.0;
        solver.x[1] = 1.0;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration < 10);
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

        assert!(solver.iteration < 60);
        assert!(ret[0].1 < 0.0001);
        // assert!(solver.x[0] < 0.1);
    }
}
