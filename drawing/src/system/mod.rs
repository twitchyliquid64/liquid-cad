mod terms;
use slotmap::HopSlotMap;
pub use terms::{TermAllocator, TermRef, TermType};

use gomez::nalgebra as na;
use gomez::prelude::*;
use na::{Dim, IsContiguous};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct Solver {
    known: HashMap<eq::Variable, eq::Concrete>,
    residuals: Vec<(TermRef, usize, eq::Expression, f64)>,
}

impl Solver {
    pub fn new(
        known: HashMap<eq::Variable, eq::Concrete>,
        residuals: Vec<(TermRef, usize, eq::Expression, f64)>,
    ) -> Self {
        Self { known, residuals }
    }

    // TODO: real error type
    pub fn solve(self) -> Result<Vec<(TermRef, f64)>, ()> {
        // Initial guess. Good choice helps the convergence of numerical methods.
        let mut x = na::DVector::from_iterator(
            self.residuals.len(),
            self.residuals.iter().map(|(_, _, _, initial)| *initial),
        );

        // Residuals vector.
        let mut fx = na::DVector::from_element(self.residuals.len(), 5555.);

        for (t_ref, num_eqs, expr, _) in self.residuals.iter() {
            println!("{}.residual = {}", t_ref, expr);
        }

        let dom = self.domain();
        let mut solver = gomez::solver::TrustRegion::new(&self, &dom);

        use gomez::core::Solver;
        for i in 1.. {
            // Do one iteration in the solving process.
            solver.next(&self, &dom, &mut x, &mut fx).map_err(|e| {
                println!("solver err: {:?}", e);
            })?;

            println!(
                "iter = {}\t|| fx || = {}\tx = {:?}",
                i,
                fx.norm(),
                x.as_slice()
            );

            // Check the termination criteria.
            if fx.norm() < 1e-6 {
                break;
            } else if i == 100 {
                return Err(());
            }
        }

        Ok(x.into_iter()
            .enumerate()
            .map(|(i, v)| (self.residuals[i].0.clone(), *v))
            .collect())
    }
}

struct VarResolver<'a, Sx>
where
    Sx: na::storage::Storage<f64, na::Dynamic>,
{
    system: &'a Solver,
    guess: &'a na::Vector<f64, na::Dynamic, Sx>,
}

impl<'a, Sx> eq::Resolver for VarResolver<'a, Sx>
where
    Sx: na::storage::Storage<f64, na::Dynamic> + IsContiguous,
{
    fn resolve_variable(&mut self, v: &eq::Variable) -> Result<eq::Concrete, eq::ResolveErr> {
        match self.system.known.get(v) {
            Some(c) => {
                return Ok(c.clone());
            }
            None => {}
        };

        for (i, (t, _, _, _)) in self.system.residuals.iter().enumerate() {
            let v2: eq::Variable = t.into();
            if v == &v2 {
                return Ok(eq::Concrete::Float(self.guess[i] as f64));
            }
        }

        Err(eq::ResolveErr::UnknownVar(v.clone()))
    }
}

impl Problem for Solver {
    type Scalar = f64;
    type Dim = na::Dynamic;
    fn dim(&self) -> Self::Dim {
        na::Dynamic::from_usize(self.residuals.len())
    }
}

impl System for Solver {
    fn eval<Sx, Sfx>(
        &self,
        x: &na::Vector<Self::Scalar, Self::Dim, Sx>,
        fx: &mut na::Vector<Self::Scalar, Self::Dim, Sfx>,
    ) -> Result<(), ProblemError>
    where
        Sx: na::storage::Storage<Self::Scalar, Self::Dim> + IsContiguous,
        Sfx: na::storage::StorageMut<Self::Scalar, Self::Dim>,
    {
        let mut resolver = VarResolver {
            guess: x,
            system: self,
        };

        for (i, (term, count, residual, _)) in self.residuals.iter().enumerate() {
            let solutions = residual.num_solutions();
            for j in 0..solutions {
                use num::traits::cast::ToPrimitive;
                let res = match residual.evaluate(&mut resolver, j).unwrap() {
                    eq::Concrete::Float(f) => f as f64,
                    eq::Concrete::Rational(r) => r.to_f64().unwrap(),
                };
                if res.is_nan() && j < solutions {
                    continue;
                }
                fx[i] = (*count as f64 * x[i]) - res;

                println!("fx[{}] = {} with guess {}", i, fx[i], x[i]);
                break;
            }
        }

        Ok(())
    }
}
