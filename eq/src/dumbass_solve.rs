extern crate nalgebra as na;
use super::*;
use crate::solve::VarResolver;
use na::{DMatrix, DVector, Dyn, OMatrix, OVector};
use num::ToPrimitive;
use std::collections::HashMap;

pub fn sigmoid(v: f64) -> f64 {
    1.0 / (1.0 + f64::exp(-v))
}

/// Hyperparameters for the DumbassSolver.
#[derive(Clone, Debug)]
pub struct DumbassSolverParams {
    /// The maximum number of iterations.
    pub max_iter: usize,
    /// A multiplier for how much the jacobian contributes to
    /// the adjustment.
    pub step_mul: f64,

    /// How much to increase the learning rate by if the gradient
    /// we are descending hasn't changed shape.
    pub momentum_step: f64,
    /// A divisor for the momentum increment, itself incremented every
    /// time the curve we are descending changes shape (i.e. we overshot
    /// the solution).
    pub momentum_div: usize,
    /// The initial value for momentum.
    pub momentum_windup: f64,

    /// The average error for all residuals at which we terminate iterations
    /// and consider the system solved.
    pub terminate_at_avg_fx: f64,
}

impl Default for DumbassSolverParams {
    fn default() -> Self {
        Self {
            max_iter: 530,
            step_mul: -0.99,
            momentum_step: 0.5,
            momentum_div: 2,
            momentum_windup: 0.15,
            terminate_at_avg_fx: 0.0005,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Jacobian {
    Func(Expression),
    Float(f64),
}

#[derive(Default, Clone, Debug)]
pub struct DumbassSolverState {
    resolved: HashMap<Variable, Concrete>,

    vars: Vec<Variable>,
    residuals: Vec<Expression>,
    jacobians: Vec<Jacobian>,
}

impl DumbassSolverState {
    pub fn new(
        concrete: HashMap<Variable, Concrete>,
        solve_for: Vec<Variable>,
        mut residuals: Vec<Expression>,
    ) -> Self {
        let jacobians: Vec<Jacobian> = solve_for
            .iter()
            .map(|var| {
                residuals.iter().map(move |fx| {
                    let jfx = fx.derivative_wrt(&var);
                    match jfx {
                        Expression::Integer(i) => Jacobian::Float(i.to_f64().unwrap()),
                        Expression::Rational(r, _) => Jacobian::Float(r.to_f64().unwrap()),
                        _ => Jacobian::Func(jfx),
                    }
                })
            })
            .flatten()
            .collect();

        for r in residuals.iter_mut() {
            let mut needs_scaling = false;
            let mut var: Option<Variable> = None;
            r.walk(&mut |e| match e {
                Expression::Variable(v) => {
                    // Hack to find residuals for the global angle
                    if v.starts_with("c") || v.starts_with("s") {
                        needs_scaling = true;
                        var = Some(v.clone());
                        false
                    } else {
                        true
                    }
                }
                _ => true,
            });

            if needs_scaling {
                let original = r.clone();
                let v = "d".to_string() + &var.unwrap()[1..];
                *r = Expression::Product(
                    Box::new(Expression::Product(
                        Box::new(Expression::Variable(v.as_str().into())),
                        Box::new(Expression::Rational(
                            Rational::new(9.into(), 10.into()),
                            true,
                        )),
                    )),
                    Box::new(original),
                );
            }
            // println!("residual: {}", r);
        }

        Self {
            resolved: concrete,
            vars: solve_for,
            residuals,
            jacobians,
        }
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
/// Whats the right direction and right amount? thats based on the
/// jacobian of the residual function with respect to each variable we are
/// trying to solve for. Don't be scared by the 'jacobian' term, it
/// basically just means the derivative with respect to a variable. So
/// it represents the 'slope' of how much the correctness of the function
/// contributes to some variable, so thats where we get our
/// 'right direction' and 'right amount' information.
///
/// For instance, lets say our residual function is: 5 - x (AKA the
/// correct solution for x is 5). The derivative with respect to
/// x is just: -x. Thats all that means.
///
/// So yeah, we work out the jacobian of the residual functions with the
/// current guesses, and we work out the error of the residual functions.
/// We multiply those together with some system of multipliers, adjust them using
/// softmax so the value is fairly distributed across the residuals, and add that
/// to the current guesses to get the guesses for the next iteration.
///
/// In machine-learning land this system of multipliers is called the
/// 'learning rate'. If the jacobians are not oscillating around some
/// local minima, we increment it slightly so we learn faster and hence
/// reach the solution sooner - the kids call this 'adaptive'. If we do
/// end up racing past a solution (as determined by the sign of any jacobian
/// changing), we increment a divisor for our learning rate increment
/// so we build up momentum slower.
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
    params: DumbassSolverParams,
    iteration: usize,

    // guess of each variable
    x: OVector<f64, Dyn>,
    // residual calculation result
    fx: OVector<f64, Dyn>,
    // jacobian by [variable, residual]
    j: OMatrix<f64, Dyn, Dyn>,

    // sign bitfield of adjustment at last iteration
    adj_sign_hash: Option<usize>,
    // accumulated momentum to add to multiplier, incremented by
    // (MOMENTUM_STEP+MOMENTUM_DIV / (resets+MOMENTUM_DIV))
    momentum: f64,
    momentum_div: usize,
}

impl DumbassSolver {
    pub fn new(st: &DumbassSolverState) -> Self {
        let params = DumbassSolverParams::default();
        let iteration = 0;

        Self {
            iteration,
            x: DVector::from_element(st.vars.len(), -8.001),
            fx: DVector::from_element(st.residuals.len(), 0.0),
            j: DMatrix::from_element(st.residuals.len(), st.vars.len(), 0.0),
            adj_sign_hash: None,
            momentum: params.momentum_windup,
            momentum_div: params.momentum_div,
            params,
        }
    }

    pub fn new_with_initials(
        params: DumbassSolverParams,
        st: &DumbassSolverState,
        initials: Vec<f64>,
    ) -> Self {
        let mut out = Self::new(st);
        out.x = DVector::from(initials);
        out.params = params;
        out
    }

    fn solve_step(&mut self, st: &mut DumbassSolverState) -> f64 {
        let DumbassSolver { x, fx, j, .. } = self;

        let mut resolver = VarResolver {
            x: &x,
            vars: &st.vars,
            resolved: &st.resolved,
            lookup: None,
        };

        // Compute jacobian
        for (i, j) in j.iter_mut().enumerate() {
            // SAFETY: st.jacobians constructed such to have
            // correct length, see DumbassSolverState::new
            let j_fn = unsafe { st.jacobians.get_unchecked(i) };

            let mut v = match j_fn {
                Jacobian::Float(f) => *f,
                Jacobian::Func(j_fn) => match j_fn.evaluate_1(&mut resolver) {
                    Ok(f) => match f {
                        Concrete::Float(f) => f as f64,
                        Concrete::Rational(r) => r.to_f64().unwrap(),
                    },
                    Err(ResolveErr::DivByZero) => 0.0,
                    Err(e) => panic!("err: {:?}", e),
                },
            };
            // TODO: These conditionals are not quite right
            if v.is_nan() {
                v = 0.;
            } else if v.is_infinite() {
                v = v.signum();
            }
            *j = v;
        }

        // Softmax the jacobian for each variable, multiplied by
        // the proportion of variables which are non-zero
        let total_terms = st.vars.len() as f64;
        for mut col in j.column_iter_mut() {
            let exp_sum = col
                .iter()
                .fold(0.0, |acc, x| acc + fast_math::exp(*x as f32) as f64);
            let terms_non_zero = col
                .iter()
                .map(|j| *j == 0.0)
                .fold(0.0, |acc, zero| acc + if zero { 0.0 } else { 1.0 });
            for j in col.iter_mut() {
                *j *= fast_math::exp(*j as f32) as f64 / exp_sum * terms_non_zero / total_terms;
            }
        }

        // Compute residuals
        for (row, exp) in st.residuals.iter().enumerate() {
            let mut res = match exp.evaluate_1(&mut resolver).unwrap() {
                Concrete::Float(f) => f as f64,
                Concrete::Rational(r) => r.to_f64().unwrap(),
            };
            if res.is_nan() {
                res = f64::INFINITY;
            }
            fx[row] = res.clamp(-999999.0, 999999.0);
        }

        // Compute total error
        let total_fx = fx.iter().fold(0.0, |acc, x| acc + x.abs());

        // println!(
        //     "x:{}j:{}fx:{}",
        //     x,
        //     j,
        //     fx
        // );

        // Compute adjustment
        let adjustment = (fx.transpose() * &*j).transpose() * self.params.step_mul;

        // Compute sign hash
        let sign_hash = adjustment.iter().enumerate().fold(0, |acc, (i, x)| {
            acc | if x.signum() == 1.0 { 1 } else { 0 } << i
        });
        // println!(
        //     "{}: sign_hash: {} -- {}",
        //     self.iteration, sign_hash, self.momentum
        // );

        // Compute momentum - revert accumulation if any jacobian changed sign
        if let Some(last_sign_hash) = self.adj_sign_hash {
            if last_sign_hash == sign_hash {
                self.momentum += self.params.momentum_step / self.momentum_div as f64;
            } else {
                self.momentum = 0.0;
                self.momentum_div += 1;
            }
        }
        self.adj_sign_hash = Some(sign_hash);

        // Update guesses
        *x += adjustment * (1.0 + self.momentum);

        total_fx
    }

    pub fn solve(
        &mut self,
        st: &mut DumbassSolverState,
    ) -> Result<Vec<(Variable, f64)>, (f64, Vec<(Variable, f64)>)> {
        let mut total_fx = f64::MAX;
        while self.iteration < self.params.max_iter {
            total_fx = self.solve_step(st);

            if (total_fx.abs() / st.vars.len() as f64) < self.params.terminate_at_avg_fx {
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
        if self.iteration < self.params.max_iter {
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
            vec![
                Jacobian::Func(
                    Expression::parse(
                        "-((x1 - x0) / sqrt((((x1 - x0))^2 + ((y1 - y0))^2)))",
                        false
                    )
                    .unwrap()
                ),
                Jacobian::Func(
                    Expression::parse(
                        "-((y1 - y0) / sqrt((((x1 - x0))^2 + ((y1 - y0))^2)))",
                        false
                    )
                    .unwrap()
                ),
            ],
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

        assert!(solver.iteration <= 8);
        assert!(ret[0].1 < 0.1);
        let f = (ret[0].1 + ret[1].1).abs();
        assert!(f > 4.9 && f < 5.1); // trashy check but gets the point across.

        // Different initial conditions solve towards a proportionally-biased solution.
        solver = DumbassSolver::new(&state);
        solver.x[0] = 1.0;
        solver.x[1] = 1.0;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration <= 8);
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

        assert!(solver.iteration < 50);
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

        assert!(solver.iteration < 50);
        let dist_leg_1 = (ret[0].1.powi(2) + ret[1].1.powi(2)).sqrt();
        assert!(dist_leg_1 > 87.9 && dist_leg_1 < 88.1);

        // Now test we can solve it in the bad quadrant
        solver = DumbassSolver::new(&state);
        solver.x[0] = 62.0;
        solver.x[1] = 62.0;
        solver.x[0] = 800.0;
        solver.x[1] = 800.0;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration < 70);
        let dist_leg_1 = (ret[0].1.powi(2) + ret[1].1.powi(2)).sqrt();
        assert!(dist_leg_1 > 87.9 && dist_leg_1 < 88.1);
    }

    #[test]
    fn simple() {
        let mut state = DumbassSolverState::new(
            HashMap::from([("y0".into(), Concrete::Float(0.0))]),
            vec!["y1".into()],
            vec![Expression::parse("5 - (0.5 * sqrt((y0 - y1)^2))", false).unwrap()],
        );
        let mut solver = DumbassSolver::new(&state);

        // Set some initial conditions.
        solver.x[0] = 0.001;
        let ret = solver.solve(&mut state).unwrap();

        assert!(solver.iteration <= 80);
        assert!((10.0 - ret[0].1).abs() < 0.001);
    }
}
