mod terms;
use slotmap::HopSlotMap;
pub use terms::{TermAllocator, TermRef, TermType};

#[derive(Debug, Clone)]
pub struct ResidualEq {
    pub(crate) standalone: bool,
    pub(crate) term: TermRef,
    pub(crate) rhs: eq::Expression,
}

impl ResidualEq {
    pub fn new(standalone: bool, term: TermRef, rhs: eq::Expression) -> Self {
        ResidualEq {
            standalone,
            term,
            rhs,
        }
    }
}

pub trait ConstraintProvider<I>
where
    I: std::iter::Iterator<Item = ResidualEq>,
{
    fn residuals(
        &self,
        features: &mut HopSlotMap<crate::FeatureKey, crate::Feature>,
        allocator: &mut TermAllocator,
    ) -> I;
}

pub fn unique_unknowns(residuals: &Vec<ResidualEq>) -> Vec<TermRef> {
    use std::collections::HashSet;
    let mut seen: HashSet<TermRef> = HashSet::with_capacity(residuals.len());
    let mut out: Vec<TermRef> = Vec::with_capacity(residuals.len());

    for r in residuals.iter() {
        match seen.get(&r.term) {
            Some(_) => {}
            None => {
                seen.insert(r.term.clone());
                out.push(r.term.clone());
            }
        }
    }

    out
}

use gomez::nalgebra as na;
use gomez::prelude::*;
use na::{Dim, IsContiguous};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct Solver {
    known: HashMap<eq::Variable, eq::Concrete>,
    residuals: Vec<(TermRef, Vec<ResidualEq>, f64)>,
}

impl Solver {
    pub fn new(
        known: HashMap<eq::Variable, eq::Concrete>,
        residuals: Vec<(TermRef, Vec<ResidualEq>, f64)>,
    ) -> Self {
        Self { known, residuals }
    }

    // TODO: real error type
    pub fn solve(&self) -> Result<Vec<f64>, ()> {
        // Initial guess. Good choice helps the convergence of numerical methods.
        let mut x = na::DVector::from_iterator(
            self.residuals.len(),
            self.residuals.iter().map(|(_, _, initial)| *initial),
        );

        // Residuals vector.
        let mut fx = na::DVector::from_element(self.residuals.len(), 5555.);

        for (t_ref, constraints, _) in self.residuals.iter() {
            print!("{}.residual = ", t_ref);
            for (i, c) in constraints.iter().enumerate() {
                print!("{}", c.rhs);
                if i < constraints.len() - 1 {
                    print!(" + ");
                }
            }
            println!();
        }

        let dom = self.domain();
        let mut solver = gomez::solver::TrustRegion::new(self, &dom);

        use gomez::core::Solver;
        for i in 1.. {
            // Do one iteration in the solving process.
            solver.next(self, &dom, &mut x, &mut fx).map_err(|e| {
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

        Ok(x.into_iter().map(|v| *v).collect())
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

        for (i, (t, _, _)) in self.system.residuals.iter().enumerate() {
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

        for (i, (term, residuals, _)) in self.residuals.iter().enumerate() {
            let mut residual = 0.;
            for r in residuals.iter() {
                use num::traits::cast::ToPrimitive;
                let res = match r.rhs.evaluate(&mut resolver, 0).unwrap() {
                    eq::Concrete::Float(f) => f as f64,
                    eq::Concrete::Rational(r) => r.to_f64().unwrap(),
                };

                if r.standalone {
                    println!("fx[{}] = {} with guess {}", i, res, x[i]);
                    residual += res.abs();
                } else {
                    println!("fx[{}] += {} with guess {}", i, x[i] - res, x[i]);
                    residual += (x[i] - res).abs();
                }
            }

            fx[i] = 2.0 * residual;
        }

        Ok(())
    }
}
