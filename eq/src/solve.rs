use super::*;
use std::collections::HashMap;
use std::rc::Rc;

/// Describes a set of expressions which represent a variable.
#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) struct EquivalentExpressions {
    exprs: Rc<Vec<ExpressionInfo>>,
}

/// Describes an expression.
#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) struct ExpressionInfo {
    expr_hash: u64,
    expr: Expression,
    cost: usize,

    references: HashMap<Variable, usize>,
}

impl From<Expression> for ExpressionInfo {
    fn from(exp: Expression) -> Self {
        let mut references: HashMap<Variable, usize> = HashMap::with_capacity(4); // 4 arbitrarily chosen
        let mut cost = exp.cost();

        exp.walk(&mut |e| {
            if let Expression::Variable(v) = e {
                match references.get_mut(&v) {
                    Some(count) => *count += 1,
                    None => {
                        references.insert(v.clone(), 1);
                        cost += 50;
                    }
                }
            }
            true
        });

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut s = DefaultHasher::new();
        exp.hash(&mut s);
        let expr_hash = s.finish();

        Self {
            expr_hash,
            expr: exp,
            cost,
            references,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct SubSolverState {
    // finite values provided or solved for.
    pub(crate) resolved: HashMap<Variable, Concrete>,
    // expressions expected to be ordered in increasing complexity.
    pub(crate) vars_by_eq: HashMap<Variable, EquivalentExpressions>,

    // tuple of (variable, expression_info) which rearranges have been attempted
    pub(crate) tried_rearrange: HashMap<(Variable, u64), ()>,
}

impl SubSolverState {
    pub fn new(
        values: HashMap<Variable, Concrete>,
        exprs: Vec<Expression>,
    ) -> Result<Self, ResolveErr> {
        let mut vars_by_eq: HashMap<Variable, EquivalentExpressions> =
            HashMap::with_capacity(exprs.len());

        // Collect equations:
        //  - <var> = <expression> straight into the map with each var as the key.
        //  -     0 = <expression> rearrange for a variable then into the map.
        for (var, expr) in exprs
            .iter()
            .map(|e| match e {
                Expression::Equal(a, b) => match a.as_ref() {
                    Expression::Variable(v) => Some((v.clone(), (**b).clone().into())),
                    Expression::Integer(i) => {
                        if i == &Integer::from(0) {
                            let mut rearranged = None;
                            e.walk(&mut |ve| {
                                if rearranged.is_some() {
                                    return false;
                                }
                                if let Expression::Variable(v) = ve {
                                    match e.make_subject(&Expression::Variable(v.clone())) {
                                        Ok(eq) => {
                                            if let Expression::Equal(_, eq) = eq {
                                                rearranged = Some((v.clone(), (*eq).into()));
                                            } else {
                                                unreachable!();
                                            }
                                            false
                                        }
                                        Err(_) => true,
                                    }
                                } else {
                                    true
                                }
                            });
                            rearranged
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
                _ => None,
            })
            .filter_map(|s| s)
        {
            if let Some(ee) = vars_by_eq.get_mut(&var) {
                Rc::get_mut(&mut ee.exprs).unwrap().push(expr);
            } else {
                vars_by_eq.insert(
                    var,
                    EquivalentExpressions {
                        exprs: Rc::new(vec![expr]),
                    },
                );
            }
        }

        // Sort the equations for a variable by increasing cost.
        for (_, v) in vars_by_eq.iter_mut() {
            Rc::get_mut(&mut v.exprs)
                .unwrap()
                .sort_by(|a, b| a.cost.cmp(&b.cost));
        }

        Ok(Self {
            vars_by_eq,
            resolved: values,
            ..SubSolverState::default()
        })
    }
}

impl super::Resolver for SubSolverState {
    fn resolve_variable(&mut self, v: &Variable) -> Result<Concrete, ResolveErr> {
        match self.resolved.get(v) {
            None => Err(ResolveErr::UnknownVar(v.clone())),
            Some(c) => Ok(c.clone()),
        }
    }
}

/// Iterative substitution solver.
#[derive(Default, Clone, Debug)]
pub struct SubSolver;

impl SubSolver {
    // Tries to solve the given expression by substituting known values in st.resolved.
    // Returns Err(ResolveErr::UnknownVar) if necessary values aren't known.
    fn find_iterative_using_exp(
        &mut self,
        st: &mut SubSolverState,
        var: &Variable,
        info: &ExpressionInfo,
    ) -> Result<Concrete, ResolveErr> {
        // println!("find_iterative_using_exp({:?}, {:?})", var, info);

        // See if we have concrete values for the dependent variables, solving them otherwise.
        for dependent_var in info.references.keys() {
            if st.resolved.get(dependent_var).is_none() {
                match self.find(st, dependent_var) {
                    Err(e) => return Err(e),
                    Ok(_) => {}
                }
            }
        }

        // If we got this far, we have all the dependent variables. Solve for the value.
        return match info.expr.evaluate(st) {
            Ok(c) => {
                st.resolved.insert(var.clone(), c.clone());
                Ok(c)
            }
            Err(e) => {
                // println!("eval error: {:?} -- {:?}", &info.expr, e);
                Err(e)
            }
        };
    }

    fn find_iterative(
        &mut self,
        st: &mut SubSolverState,
        var: &Variable,
    ) -> Result<Concrete, ResolveErr> {
        // println!("find_iterative({:?})", var);

        // Find the expressions to solve for the target var
        match st.vars_by_eq.get(var) {
            Some(ee) => {
                for info in ee.exprs.clone().as_ref() {
                    match self.find_iterative_using_exp(st, var, info) {
                        Ok(c) => return Ok(c),
                        Err(ResolveErr::UnknownVar(_)) => continue,
                        Err(e) => return Err(e),
                    }
                }
            }
            None => {}
        }

        Err(ResolveErr::CannotSolve)
    }

    fn rearrange_candidate(
        &mut self,
        st: &mut SubSolverState,
        var: &Variable,
    ) -> Result<ExpressionInfo, ResolveErr> {
        // println!("rearrange_candidate({:?})", var);

        for (lhs_var, ee) in st.vars_by_eq.iter() {
            for info in ee.exprs.iter() {
                if info.references.get(var).is_some() {
                    // See if we've already tried this before
                    {
                        let k = (var.clone(), info.expr_hash);
                        if st.tried_rearrange.get(&k).is_some() {
                            continue;
                        }
                        st.tried_rearrange.insert(k, ());
                    }

                    let v = Expression::Variable(lhs_var.clone());
                    let eq = Expression::Equal(Box::new(v.clone()), Box::new(info.expr.clone()));

                    match eq.make_subject(&Expression::Variable(var.clone())) {
                        Ok(eq) => {
                            if let Expression::Equal(_, eq) = eq {
                                let ee: ExpressionInfo = (*eq).into();
                                return Ok(ee);
                            } else {
                                unreachable!();
                            }
                        }
                        Err(_e) => {
                            // println!("cannot rearrange {:?}, continuing ({:?})", &eq, e)
                        }
                    }
                }
            }
        }

        Err(ResolveErr::CannotSolve)
    }

    pub fn find_all<'a>(&mut self, st: &'a mut SubSolverState) -> &'a HashMap<Variable, Concrete> {
        let mut vars: Vec<Variable> = st.vars_by_eq.iter().map(|(v, _)| v.clone()).collect();
        for (_v, ees) in st.vars_by_eq.iter() {
            for e in ees.exprs.iter() {
                e.expr.walk(&mut |e| {
                    if let Expression::Variable(v) = e {
                        if !vars.contains(v) {
                            vars.push(v.clone());
                        }
                    }
                    true
                });
            }
        }

        for v in vars {
            if let None = st.resolved.get(&v) {
                self.find(st, &v);
            }
        }

        &st.resolved
    }

    pub fn find(
        &mut self,
        st: &mut SubSolverState,
        var: &Variable,
    ) -> Result<Concrete, ResolveErr> {
        // println!("find({:?})", var);

        // First, see if we know the value of the requested variable.
        if let Some(val) = st.resolved.get(var) {
            return Ok(val.clone());
        }

        // Next, if theres an equation that resolves that variable, try and solve it,
        // iteratively solving dependent variables.
        match self.find_iterative(st, var) {
            Ok(c) => return Ok(c),
            Err(ResolveErr::UnknownVar(_)) => {}
            Err(ResolveErr::CannotSolve) => {}
            Err(e) => return Err(e),
        }

        // Lastly, if none of that worked, try to rearrange an equation for the requested
        // variable and recurse to solve that.
        loop {
            match self.rearrange_candidate(st, var) {
                Ok(ee) => {
                    // println!("got rearrange candidate: {:?}", &ee);
                    match self.find_iterative_using_exp(st, var, &ee) {
                        Ok(c) => return Ok(c),
                        Err(ResolveErr::UnknownVar(_)) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    #[test]
    fn new() {
        // concrete values assigned to resolved
        assert!(SubSolverState::new(
            HashMap::from([(
                "a".into(),
                Concrete::Rational(Rational::new(1.into(), 2.into()))
            )]),
            vec![]
        )
        .unwrap()
        .resolved
        .get(&"a".into())
        .is_some());

        // expressions assigned to vars_by_eq
        assert_eq!(
            SubSolverState::new(
                HashMap::new(),
                vec![
                    Expression::parse("a = x+1", false).unwrap(),
                    Expression::parse("a = y/2", false).unwrap(),
                ]
            )
            .unwrap()
            .vars_by_eq,
            HashMap::from([(
                Variable::from("a"),
                EquivalentExpressions {
                    exprs: Rc::new(vec![
                        Expression::parse("x+1", false).unwrap().into(),
                        Expression::parse("y/2", false).unwrap().into()
                    ]),
                }
            ),]),
        );

        // residual expressions assigned to vars_by_eq
        assert_eq!(
            SubSolverState::new(
                HashMap::new(),
                vec![
                    Expression::parse("0 = x+1 - a", false).unwrap(),
                    Expression::parse("0 = y/2 - a", false).unwrap(),
                ]
            )
            .unwrap()
            .vars_by_eq,
            HashMap::from([
                (
                    Variable::from("x"),
                    EquivalentExpressions {
                        exprs: Rc::new(vec![Expression::parse("a-1", false).unwrap().into(),]),
                    }
                ),
                (
                    Variable::from("y"),
                    EquivalentExpressions {
                        exprs: Rc::new(vec![Expression::parse("2a", false).unwrap().into(),]),
                    }
                )
            ]),
        );
    }

    #[test]
    fn cached_value() {
        let mut state = SubSolverState::new(
            HashMap::from([(
                "a".into(),
                Concrete::Rational(Rational::new(1.into(), 2.into())),
            )]),
            vec![],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"a".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::new(1.into(), 2.into())),
            _ => panic!("result is not a rational"),
        }
    }

    #[test]
    fn simple() {
        let mut state = SubSolverState::new(
            HashMap::from([(
                "a".into(),
                Concrete::Rational(Rational::new(1.into(), 2.into())),
            )]),
            vec![
                Expression::parse("b = a", false).unwrap(),
                Expression::parse("c = 3b", false).unwrap(),
            ],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"c".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::new(3.into(), 2.into())),
            _ => panic!("result is not a rational"),
        }
    }

    #[test]
    fn solve_rect() {
        // rectangle with point 0 at (0, 0), with the other points defined
        // by a width and height, and horizontal / vertical lines.
        let mut state = SubSolverState::new(
            HashMap::from([
                (
                    "x0".into(),
                    Concrete::Rational(Rational::from_integer(0.into())),
                ),
                (
                    "y0".into(),
                    Concrete::Rational(Rational::from_integer(0.into())),
                ),
                (
                    "w".into(),
                    Concrete::Rational(Rational::from_integer(5.into())),
                ),
                (
                    "h".into(),
                    Concrete::Rational(Rational::from_integer(10.into())),
                ),
            ]),
            vec![
                Expression::parse("x1 = x0", false).unwrap(),
                Expression::parse("y1 = y0 + h", false).unwrap(),
                Expression::parse("y2 = y1", false).unwrap(),
                Expression::parse("x2 = x1 + w", false).unwrap(),
                Expression::parse("x3 = x2", false).unwrap(),
                Expression::parse("y3 = y0", false).unwrap(),
            ],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"x1".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(0.into())),
            _ => panic!("result is not a rational"),
        }
        match SubSolver::default().find(&mut state, &"y1".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(10.into())),
            _ => panic!("result is not a rational"),
        }
        match SubSolver::default().find(&mut state, &"x2".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(5.into())),
            _ => panic!("result is not a rational"),
        }
        match SubSolver::default().find(&mut state, &"y2".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(10.into())),
            _ => panic!("result is not a rational"),
        }
        match SubSolver::default().find(&mut state, &"x3".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(5.into())),
            _ => panic!("result is not a rational"),
        }
        match SubSolver::default().find(&mut state, &"y3".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(0.into())),
            _ => panic!("result is not a rational"),
        }
    }

    #[test]
    fn solve_needing_rearrange() {
        let mut state = SubSolverState::new(
            HashMap::from([(
                "a".into(),
                Concrete::Rational(Rational::from_integer(6.into())),
            )]),
            vec![
                Expression::parse("b = a", false).unwrap(),
                Expression::parse("b = 2 * (c+1)", false).unwrap(),
            ],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"c".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(2.into())),
            _ => panic!("result is not a rational"),
        }
    }

    #[test]
    fn solve_terminates() {
        let mut state = SubSolverState::new(
            HashMap::from([(
                "a".into(),
                Concrete::Rational(Rational::from_integer(6.into())),
            )]),
            vec![
                Expression::parse("d = c / 2", false).unwrap(),
                Expression::parse("b = a + d", false).unwrap(),
                Expression::parse("b = 2 * (c+1)", false).unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(
            SubSolver::default().find(&mut state, &"c".into()).err(),
            Some(ResolveErr::CannotSolve),
        );
    }

    #[test]
    fn solve_line_slope() {
        //      p1-----line 1-----p2
        //     /                 /
        //    /                 /
        //  line 0           line 2
        //  /                 /
        // p0-----line 3-----p3
        //
        // line 1&3 are horizontal, therefore: p1.y == p2.y, p0.y == p3.y
        // line 1&3 have fixed distance of 5

        let mut state = SubSolverState::new(
            HashMap::from([
                (
                    "x0".into(),
                    Concrete::Rational(Rational::from_integer(0.into())),
                ),
                (
                    "y0".into(),
                    Concrete::Rational(Rational::from_integer(1.into())),
                ),
                (
                    "x1".into(),
                    Concrete::Rational(Rational::from_integer(1.into())),
                ),
                (
                    "y1".into(),
                    Concrete::Rational(Rational::from_integer(11.into())),
                ),
            ]),
            vec![
                Expression::parse("y2 = y1", false).unwrap(),
                Expression::parse("y3 = y0", false).unwrap(),
                Expression::parse("x2 = x1 + 5", false).unwrap(),
                Expression::parse("x3 = x0 + 5", false).unwrap(),
                Expression::parse("m0 = (y1-y0)/(x1-x0)", false).unwrap(),
                Expression::parse("m3 = (y2-y3)/(x2-x3)", false).unwrap(),
            ],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"y3".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(1.into())),
            _ => panic!("result is not a rational"),
        }
        match SubSolver::default().find(&mut state, &"x3".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(5.into())),
            _ => panic!("result is not a rational"),
        }
    }

    #[test]
    fn solve_aligned_distance() {
        // p0-----line 1-----p1
        //
        // line 1 is horizontal, therefore: p0.y == p1.y
        // line 1 has fixed distance of 5
        // p0 is at (0, 1)

        let mut state = SubSolverState::new(
            HashMap::from([
                (
                    "x0".into(),
                    Concrete::Rational(Rational::from_integer(0.into())),
                ),
                (
                    "y0".into(),
                    Concrete::Rational(Rational::from_integer(1.into())),
                ),
            ]),
            vec![
                Expression::parse("y1 = y0", false).unwrap(),
                Expression::parse("d1 = sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap(),
                Expression::parse("d1 = 5", false).unwrap(),
            ],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"x1".into()).unwrap() {
            Concrete::Float(x) => assert_eq!(x, 5.0),
            _ => panic!("result is not a float"),
        }
        match SubSolver::default().find(&mut state, &"y1".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(1.into())),
            _ => panic!("result is not a rational"),
        }

        // p0
        // |
        // | line 1
        // |
        // p1
        //
        // line 1 is vertical, therefore: p0.x == p1.x
        // line 1 has fixed distance of 5
        // p0 is at (0, 0)

        state = SubSolverState::new(
            HashMap::from([
                (
                    "x0".into(),
                    Concrete::Rational(Rational::from_integer(0.into())),
                ),
                (
                    "y0".into(),
                    Concrete::Rational(Rational::from_integer(0.into())),
                ),
            ]),
            vec![
                Expression::parse("x1 = x0", false).unwrap(),
                Expression::parse("d1 = sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap(),
                Expression::parse("d1 = 5", false).unwrap(),
            ],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"x1".into()).unwrap() {
            Concrete::Rational(r) => assert_eq!(r, Rational::from_integer(0.into())),
            _ => panic!("result is not a rational"),
        }
        match SubSolver::default().find(&mut state, &"y1".into()).unwrap() {
            Concrete::Float(x) => assert_eq!(x, 5.0),
            _ => panic!("result is not a float"),
        }
    }
}
