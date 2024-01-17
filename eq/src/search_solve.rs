extern crate nalgebra as na;
use super::*;
use crate::solve::VarResolver;
use na::{DVector, Dyn, OVector};
use num::ToPrimitive;
use std::collections::HashMap;

/// Brute-force search solver, iterating outwards exponentially
/// and keeping track of the best values (producing the smallest
/// least-squares sum of residuals).
#[derive(Clone, Debug)]
pub struct SearchSolver {
    resolved: HashMap<Variable, Concrete>,
    iteration: usize,

    vars: Vec<Variable>,
    residuals: Vec<Expression>,

    // guess of each variable
    x: OVector<f64, Dyn>,
    // best guesses so far
    best: (f64, OVector<f64, Dyn>),

    iterators: Vec<ExpSearchIter>,
}

impl SearchSolver {
    pub fn new<I: IntoIterator<Item = f64>>(
        params: ExpSearchParams,
        concrete: HashMap<Variable, Concrete>,
        solve_for: Vec<Variable>,
        residuals: Vec<Expression>,
        initials: I,
    ) -> Self {
        let mut iterators: Vec<ExpSearchIter> = initials
            .into_iter()
            .map(|x| ExpSearchIter::new(params.clone(), x))
            .collect();

        let x = DVector::from_iterator(
            iterators.len(),
            iterators.iter_mut().map(|i| i.next().unwrap()),
        );

        let best = (f64::INFINITY, x.clone());

        Self {
            iteration: 0,
            resolved: concrete,
            vars: solve_for,
            residuals,
            iterators,

            x,
            best,
        }
    }

    // Nice idea, wasn't stable in practice :(
    // pub fn bruteforce_in_pairs(mut self, search_bits: usize) -> (f64, Vec<(Variable, f64)>) {
    //     if 2 >= self.vars.len() {
    //         return self.bruteforce(search_bits);
    //     }

    //     let group_iters = 2usize.pow((2 * search_bits) as u32);
    //     let var_states = 2usize.pow(search_bits as u32);

    //     for _ in 0..2 {
    //         for i1 in 0..self.iterators.len() {
    //             for i2 in i1..self.iterators.len() {
    //                 self.iteration = 0;
    //                 while self.iteration < group_iters {
    //                     let v1 = (var_states - 1) & (self.iteration >> (0 * search_bits));
    //                     let v2 = (var_states - 1) & (self.iteration >> (1 * search_bits));
    //                     self.x[i1] = self.iterators[i1].set_step(v1);
    //                     self.x[i2] = self.iterators[i2].set_step(v2);
    //                     self.compute_residuals_step();
    //                 }

    //                 self.iterators[i1].set_x(self.best.1[i1]);
    //                 self.iterators[i2].set_x(self.best.1[i2]);
    //             }
    //         }
    //     }

    //     (
    //         self.best.0,
    //         self.vars
    //             .into_iter()
    //             .enumerate()
    //             .map(|(i, v)| (v.clone(), self.best.1[i]))
    //             .collect(),
    //     )
    // }

    // TODO: API that lets you do bits of work at a time
    pub fn bruteforce(mut self, search_bits: usize) -> (f64, Vec<(Variable, f64)>) {
        let total = 2usize.pow((self.vars.len() * search_bits) as u32);
        let var_states = 2usize.pow(search_bits as u32);

        while self.iteration < total {
            // Update guesses
            for (i, g) in self.iterators.iter_mut().enumerate() {
                let v = (var_states - 1) & (self.iteration >> (i * search_bits));
                // println!("{}:{}\tb{:b}", self.iteration, i, v);
                self.x[i] = g.set_step(v);
            }

            self.compute_residuals_step();
        }

        (
            self.best.0,
            self.vars
                .into_iter()
                .enumerate()
                .map(|(i, v)| (v.clone(), self.best.1[i]))
                .collect(),
        )
    }

    fn compute_residuals_step(&mut self) {
        let SearchSolver {
            x,
            best,
            vars,
            resolved,
            residuals,
            ..
        } = self;

        let mut resolver = VarResolver {
            x: &x,
            vars: &vars,
            resolved: &resolved,
            lookup: None,
        };

        // Compute residuals
        let mut sum_sq: f64 = 0.0;
        for exp in residuals.iter() {
            let res = match exp.evaluate_1(&mut resolver).unwrap() {
                Concrete::Float(f) => f as f64,
                Concrete::Rational(r) => r.to_f64().unwrap(),
            };
            let res = res.clamp(-999999.0, 999999.0);
            sum_sq += res * res;
        }

        //println!("{}", x);

        if best.0 > sum_sq {
            best.0 = sum_sq;
            best.1.copy_from(x);
        }
        self.iteration += 1;
    }
}

/// The step parameters for ExpSearchIter.
#[derive(Clone, Debug)]
pub struct ExpSearchParams {
    pub step: f64,
    pub exp: f64,
}

impl Default for ExpSearchParams {
    fn default() -> Self {
        Self {
            step: 0.707,
            exp: 1.33,
        }
    }
}

impl ExpSearchParams {
    pub fn reduce(&mut self) {
        self.step /= 10.0;
        self.exp = (1.0 as f64).max(self.exp - 0.003);
    }
}

/// An iterator that yields a scalar outwards from its
/// initial point.
#[derive(Default, Clone, Debug)]
pub struct ExpSearchIter {
    params: ExpSearchParams,
    x: f64,
    i: usize,
}

impl ExpSearchIter {
    pub fn new(params: ExpSearchParams, val: f64) -> Self {
        Self {
            params,
            x: val,
            i: 0,
        }
    }

    pub fn val(&self) -> f64 {
        let (is_pos, mul) = (self.i % 2 == 1, (self.i + 1) / 2);
        let step = (mul as f64 * self.params.step).powf(self.params.exp);

        self.x + if is_pos { step } else { -step }
    }

    pub fn reset(&mut self) {
        self.i = 0;
    }

    pub fn set_step(&mut self, i: usize) -> f64 {
        self.i = i;
        self.val()
    }

    pub fn set_x(&mut self, x: f64) {
        self.x = x;
        self.i = 0;
    }
}

impl Iterator for ExpSearchIter {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        let out = self.val();
        self.i += 1;
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter_step() {
        let mut i = ExpSearchIter::new(
            ExpSearchParams {
                step: 0.2,
                exp: 1.0,
            },
            5.0,
        );

        assert_eq!(i.next(), Some(5.0));
        assert_eq!(i.next(), Some(5.2));
        assert_eq!(i.next(), Some(4.8));
        assert_eq!(i.next(), Some(5.4));
        assert_eq!(i.next(), Some(4.6));
        assert_eq!(i.next(), Some(5.6));
        assert_eq!(i.next(), Some(4.4));
    }

    #[test]
    fn iter_exp() {
        let mut i = ExpSearchIter::new(
            ExpSearchParams {
                step: 0.1,
                exp: 2.0,
            },
            5.0,
        );

        assert_eq!(i.next(), Some(5.0));
        assert_eq!(i.next(), Some(5.01));
        assert_eq!(i.next(), Some(4.99));
        assert_eq!(i.next(), Some(5.04));
        assert_eq!(i.next(), Some(4.96));
        assert_eq!(i.next(), Some(5.09));
        assert_eq!(i.next(), Some(4.91));
    }

    #[test]
    fn search_solver_bruteforce_1dof() {
        let ss = SearchSolver::new(
            ExpSearchParams::default(),
            HashMap::new(),
            vec!["a".into()],
            vec![Expression::parse("88 - a", false).unwrap()],
            vec![85.0], // initial value
        );

        let (residual_sq, guesses) = ss.bruteforce(6);
        assert!(residual_sq < 2.0);
        assert!((88.0 - guesses[0].1).abs() < 0.3);
    }

    #[test]
    fn search_solver_bruteforce_2dof() {
        let ss = SearchSolver::new(
            ExpSearchParams::default(),
            HashMap::from([
                ("x0".into(), Concrete::Float(0.0)),
                ("y0".into(), Concrete::Float(0.0)),
            ]),
            vec!["x1".into(), "y1".into()],
            vec![Expression::parse("5 - sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap()],
            vec![8.5, 2.5], // initial value
        );

        let (residual_sq, guesses) = ss.bruteforce(6);
        assert!(residual_sq < 5.0);
        assert!((5.0 - (guesses[0].1.powi(2) + guesses[1].1.powi(2)).sqrt()) < 0.2);
    }
}
