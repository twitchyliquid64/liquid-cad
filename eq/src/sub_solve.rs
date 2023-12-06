use super::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Describes a set of expressions which represent a variable.
/// Expressions are ordered by increasing cost.
#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) struct EquivalentExpressions {
    seen: HashMap<ExprHash, usize>,
    exprs: Rc<Vec<ExpressionInfo>>,
}
impl EquivalentExpressions {
    fn from_expr(expr: Expression) -> Self {
        let ei: ExpressionInfo = expr.into();
        let mut seen = HashMap::with_capacity(16);
        let mut v = Vec::with_capacity(16); // arbitrarily chosen

        seen.insert(ei.expr_hash, 0);
        v.push(ei);

        Self {
            seen,
            exprs: Rc::new(v),
        }
    }

    fn push(&mut self, expr: Expression) {
        let ei: ExpressionInfo = expr.into();
        if self.seen.contains_key(&ei.expr_hash) {
            return;
        }
        self.seen.insert(ei.expr_hash, self.exprs.len());

        let exprs = Rc::get_mut(&mut self.exprs).unwrap();
        match exprs.binary_search(&ei) {
            Ok(_pos) => {} // already exists
            Err(pos) => exprs.insert(pos, ei),
        }
    }
}

/// Describes an expression. Ordered by cost then hash.
#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) struct ExpressionInfo {
    expr_hash: ExprHash,
    expr: Expression,
    cost: usize,

    references: HashMap<Variable, usize>,
}

impl From<Expression> for ExpressionInfo {
    fn from(exp: Expression) -> Self {
        let mut references: HashMap<Variable, usize> = HashMap::with_capacity(8); // 8 arbitrarily chosen
        let mut cost = exp.cost() * exp.num_solutions();

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

        let expr_hash = ExprHash::from(&exp);
        Self {
            expr_hash,
            expr: exp,
            cost,
            references,
        }
    }
}

impl Ord for ExpressionInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let by_cost = self.cost.cmp(&other.cost);
        match by_cost {
            std::cmp::Ordering::Equal => self.expr_hash.cmp(&other.expr_hash),
            _ => by_cost,
        }
    }
}

impl PartialOrd for ExpressionInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Describes how to solve for a variable.
#[derive(Debug, Clone)]
pub(crate) enum SolvePlan {
    Concrete(Concrete),
    Substituted(ExpressionInfo),
}

#[derive(Default, Clone, Debug)]
pub struct SubSolverState {
    done_substitution: bool,
    // finite values provided or solved for.
    resolved: HashMap<Variable, SolvePlan>,
    // expressions expected to be ordered in increasing complexity.
    vars_by_eq: HashMap<Variable, EquivalentExpressions>,
}

impl SubSolverState {
    pub fn new(values: HashMap<Variable, Concrete>, exprs: Vec<Expression>) -> Result<Self, ()> {
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
                                        }
                                        Err(_) => {}
                                    }
                                };
                                true
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
                ee.push(expr);
            } else {
                vars_by_eq.insert(var, EquivalentExpressions::from_expr(expr));
            }
        }

        let mut resolved = values
            .into_iter()
            .map(|(k, v)| (k, SolvePlan::Concrete(v)))
            .collect::<HashMap<_, _>>();
        resolved.reserve(256.max(vars_by_eq.len()));

        let done_substitution = false;

        Ok(Self {
            done_substitution,
            vars_by_eq,
            resolved,
            ..SubSolverState::default()
        })
    }
}

impl super::Resolver for SubSolverState {
    fn resolve_variable(&mut self, v: &Variable) -> Result<Concrete, ResolveErr> {
        match self.resolved.get(v) {
            None => Err(ResolveErr::UnknownVar(v.clone())),
            Some(p) => match p {
                SolvePlan::Concrete(c) => Ok(c.clone()),
                _ => Err(ResolveErr::UnknownVar(v.clone())),
            },
        }
    }
}

/// Iterative substitution solver.
#[derive(Default, Clone, Debug)]
pub struct SubSolver;

impl SubSolver {
    // Tries to solve the given expression by substituting known values in st.resolved.
    // Returns Err(ResolveErr::UnknownVar) if necessary values aren't known.
    fn solve_using_known(
        &mut self,
        st: &mut SubSolverState,
        var: &Variable,
        info: &ExpressionInfo,
    ) -> Result<SolvePlan, ResolveErr> {
        // println!("solve_using_known({:?}, {:?})", var, info);

        let mut out = info.clone();
        // Ensure we have all the dependent variables + perform substitution.
        for dependent_var in info.references.keys() {
            match st.resolved.get(&dependent_var) {
                None => {
                    return Err(ResolveErr::CannotSolve);
                }
                Some(p) => match p {
                    SolvePlan::Substituted(ei) => out
                        .expr
                        .sub_variable(dependent_var, Box::new(ei.expr.clone())),
                    SolvePlan::Concrete(_) => {}
                },
            };
        }

        // Store the equation as a resolved value.
        if !st.resolved.contains_key(var) {
            // As a special case, if the equation only has one solution
            // then we store the numeric result rather than the equation.
            if out.expr.num_solutions() == 1 {
                let cc = out.expr.evaluate(st, 0).unwrap();
                match cc {
                    Concrete::Float(ref f) if !f.is_nan() && !f.is_infinite() => {
                        st.resolved
                            .insert(var.clone(), SolvePlan::Concrete(cc.clone()));
                        return Ok(SolvePlan::Concrete(cc));
                    }
                    Concrete::Rational(_) => {
                        st.resolved
                            .insert(var.clone(), SolvePlan::Concrete(cc.clone()));
                        return Ok(SolvePlan::Concrete(cc));
                    }
                    _ => {}
                }
            }

            st.resolved
                .insert(var.clone(), SolvePlan::Substituted(out.clone()));
        }

        Ok(SolvePlan::Substituted(out))
    }

    fn rearrange_candidate(
        &mut self,
        st: &mut SubSolverState,
        var: &Variable,
    ) -> Result<ExpressionInfo, ResolveErr> {
        // println!("rearrange_candidate({:?})", var);

        for (lhs_var, ee) in st.vars_by_eq.iter() {
            // If we don't have the lhs_var, there's no point continuing as
            // we cannot solve it.
            if !st.resolved.contains_key(&lhs_var) {
                continue;
            };
            'expr_loop: for info in ee.exprs.iter() {
                // Make sure the candidate expression contains the variable we care
                // about.
                if info.references.get(var).is_some() {
                    // Make sure all the other variables referenced are known.
                    for v in info.references.keys() {
                        if v == var {
                            continue;
                        };
                        if !st.resolved.contains_key(v) {
                            continue 'expr_loop;
                        };
                    }

                    // At this stage, we should be able to re-arrange the equation
                    // to find our target var.
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
                            // println!("cannot rearrange {:?}, continuing ({:?})", &eq, var)
                        }
                    }
                }
            }
        }

        Err(ResolveErr::CannotSolve)
    }

    fn all_vars(&mut self, st: &mut SubSolverState) -> Vec<Variable> {
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
        for v in st.resolved.keys() {
            if !vars.contains(v) {
                vars.push(v.clone());
            }
        }
        SubSolver::sort_vars_by_base(&mut vars);
        vars
    }

    fn sort_vars_by_base(vars: &mut Vec<Variable>) {
        // Assuming variables of form "<letter><integer>" the sort order
        // is by integer-first.
        vars.sort_by(|a, b| match (a.as_str().get(1..), b.as_str().get(1..)) {
            (Some(a_str), Some(b_str)) => match (a_str.parse::<usize>(), b_str.parse::<usize>()) {
                (Ok(ai), Ok(bi)) => match ai.partial_cmp(&bi) {
                    Some(std::cmp::Ordering::Equal) => a.partial_cmp(b).unwrap(),
                    v => v.unwrap(),
                },
                _ => a.partial_cmp(b).unwrap(),
            },
            _ => a.partial_cmp(b).unwrap(),
        });
    }

    fn try_solve(&mut self, st: &mut SubSolverState) -> Vec<Variable> {
        let vars = self.all_vars(st);
        if st.done_substitution {
            return vars;
        }

        'outer_loop: for _i in 0..vars.len() {
            // Find the next variable which is simplest to solve.
            for v in vars.iter() {
                if st.resolved.contains_key(&v) {
                    continue;
                };
                // See if we have all the dependent variables to solve
                // one of the expressions.
                match st.vars_by_eq.get(v) {
                    Some(ee) => {
                        for info in ee.exprs.clone().as_ref() {
                            match self.solve_using_known(st, v, info) {
                                Ok(_p) => continue 'outer_loop,
                                Err(_) => continue,
                            }
                        }
                    }
                    None => {}
                }
            }
            // Oh no! There wasn't a simple substitution to be done this round.
            // Lets try rearranging equations that have the right variables
            // to be solved for the target.
            for v in vars.iter() {
                if st.resolved.contains_key(&v) {
                    continue;
                };
                if let Ok(ei) = self.rearrange_candidate(st, v) {
                    match self.solve_using_known(st, v, &ei) {
                        Ok(_p) => continue 'outer_loop,
                        Err(_) => continue,
                    }
                }
            }
        }

        st.done_substitution = true;
        vars
    }

    pub fn walk_solutions<'a>(
        &mut self,
        st: &'a mut SubSolverState,
        cb: &mut impl FnMut(&mut SubSolverState, &Variable, &Expression) -> (bool, Option<Concrete>),
    ) {
        let vars = self.try_solve(st);

        for v in vars {
            if let Some(p) = st.resolved.get(&v).clone() {
                let (keep_going, chosen_solution) = cb(
                    st,
                    &v,
                    &match p {
                        SolvePlan::Concrete(c) => match c {
                            Concrete::Float(f) => {
                                Expression::Rational(super::Rational::from_float(*f).unwrap(), true)
                            }
                            Concrete::Rational(r) => Expression::Rational(r.clone(), false),
                        },
                        SolvePlan::Substituted(e) => e.expr.clone(),
                    },
                );
                if let Some(c) = chosen_solution {
                    if matches!(c, Concrete::Rational(_)) || c.as_f64().is_normal() {
                        st.resolved.insert(v, SolvePlan::Concrete(c));
                    }
                };
                if !keep_going {
                    return;
                }
            };
        }
    }

    pub fn find(
        &mut self,
        st: &mut SubSolverState,
        var: &Variable,
    ) -> Result<Concrete, ResolveErr> {
        let mut out = None;
        self.walk_solutions(st, &mut |st, v, expr| -> (bool, Option<Concrete>) {
            if v == var {
                let output = expr.evaluate(st, 0).unwrap();
                out = Some(output.clone());
                (false, Some(output))
            } else {
                (true, None)
            }
        });

        out.ok_or(ResolveErr::CannotSolve)
    }

    pub fn all_concrete_results(
        &mut self,
        st: &mut SubSolverState,
    ) -> (HashMap<Variable, Concrete>, Vec<Variable>) {
        let vars = self.try_solve(st);
        let mut out = HashMap::with_capacity(vars.len());
        let mut unresolved = HashSet::with_capacity(vars.len());

        for v in vars {
            if let Some(SolvePlan::Concrete(c)) = st.resolved.get(&v).clone() {
                out.insert(v, c.clone());
            } else {
                unresolved.insert(v);
            }
        }

        (out, unresolved.into_iter().collect())
    }

    pub fn all_residuals(&mut self, st: &mut SubSolverState) -> Vec<Expression> {
        let mut done_exprs: HashSet<ExprHash> =
            HashSet::with_capacity(st.vars_by_eq.len().max(256));
        let mut out = Vec::with_capacity(4 * st.vars_by_eq.len());

        for (for_var, ee) in st.vars_by_eq.iter() {
            for ei in ee.exprs.iter() {
                // Skip fully-constrained residuals
                if matches!(st.resolved.get(for_var), Some(SolvePlan::Concrete(_)))
                    && ei
                        .references
                        .keys()
                        .all(|v| matches!(st.resolved.get(v), Some(SolvePlan::Concrete(_))))
                {
                    continue;
                }

                // let eq = Expression::Equal(
                //     Box::new(Expression::Variable(for_var.clone())),
                //     Box::new(ei.expr.clone()),
                // )
                // .as_residual().unwrap();
                let eq = Expression::Difference(
                    Box::new(Expression::Variable(for_var.clone())),
                    Box::new(ei.expr.clone()),
                );

                let h: ExprHash = (&eq).into();
                if done_exprs.contains(&h) {
                    continue;
                }
                done_exprs.insert(h);
                out.push((h, eq));
            }
        }

        out.sort_by(|a, b| a.0.cmp(&b.0));
        out.into_iter().map(|(_h, exp)| exp).collect()
    }

    /// all_remaining_residuals returns the set of all variables for which there is no concrete
    /// solution, and an expression representing the residual of all expressions which influence
    /// that variable.
    pub fn all_remaining_residuals(
        &mut self,
        st: &mut SubSolverState,
    ) -> HashMap<Variable, (usize, Expression)> {
        let vars = self.try_solve(st);
        let mut out = HashMap::with_capacity(vars.len());

        for var in vars {
            // If we already have a solution for this variable, continue.
            if let Some(SolvePlan::Concrete(_)) = st.resolved.get(&var) {
                continue;
            }

            // TODO: Maybe we should try and have less residuals than variables if all
            // the equations for a variable can be substituted into one represented by another residual?
            let mut exprs: Option<Expression> = None;
            let mut count: usize = 0;
            for (for_var, ee) in st.vars_by_eq.iter() {
                let mut done_exprs: HashSet<ExprHash> =
                    HashSet::with_capacity(st.resolved.len().max(256));

                if ee.exprs.len() > 0 {
                    for ei in ee.exprs.iter() {
                        if !ei.references.contains_key(&var) {
                            continue;
                        }

                        let eq = Expression::Equal(
                            Box::new(Expression::Variable(for_var.clone())),
                            Box::new(ei.expr.clone()),
                        );
                        let rearranged = match eq.make_subject(&Expression::Variable(var.clone())) {
                            Ok(eq) => {
                                if let Expression::Equal(_, eq) = eq {
                                    Some(eq)
                                } else {
                                    unreachable!();
                                }
                            }
                            Err(_e) => {
                                // println!("cannot rearrange {:?}, continuing ({:?})", &eq, var)
                                None
                            }
                        };

                        if let Some(eq) = rearranged {
                            if done_exprs.contains(&ei.expr_hash) {
                                continue;
                            }
                            done_exprs.insert(ei.expr_hash);

                            if let Some(e) = exprs {
                                exprs = Some(Expression::Sum(Box::new(e), Box::new(*eq)));
                            } else {
                                exprs = Some(*eq);
                            }
                            count += 1;
                        }
                    }
                }
            }

            if let Some(e) = exprs {
                out.insert(var.clone(), (count, e));
            }
        }

        out
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

        // expressions assigned to vars_by_eq, increasing cost
        assert_eq!(
            SubSolverState::new(
                HashMap::new(),
                vec![
                    Expression::parse("a = y/2", false).unwrap(),
                    Expression::parse("a = x+1", false).unwrap(),
                ]
            )
            .unwrap()
            .vars_by_eq[&"a".into()]
                .exprs,
            Rc::new(vec![
                Expression::parse("x+1", false).unwrap().into(),
                Expression::parse("y/2", false).unwrap().into()
            ]),
        );

        // residual expressions assigned to vars_by_eq, dedupe
        assert_eq!(
            SubSolverState::new(
                HashMap::new(),
                vec![
                    Expression::parse("0 = x+1 - a", false).unwrap(),
                    Expression::parse("0 = y/2 - a", false).unwrap(),
                    Expression::parse("0 = y/2 - a", false).unwrap(),
                ]
            )
            .unwrap()
            .vars_by_eq[&"x".into()]
                .exprs,
            Rc::new(vec![Expression::parse("a-1", false).unwrap().into(),]),
        );
        assert_eq!(
            SubSolverState::new(
                HashMap::new(),
                vec![
                    Expression::parse("0 = x+1 - a", false).unwrap(),
                    Expression::parse("0 = y/2 - a", false).unwrap(),
                ]
            )
            .unwrap()
            .vars_by_eq[&"y".into()]
                .exprs,
            Rc::new(vec![Expression::parse("2a", false).unwrap().into(),]),
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
                Expression::parse("c = b", false).unwrap(),
                Expression::parse("c = 2 * (d+1)", false).unwrap(),
            ],
        )
        .unwrap();

        match SubSolver::default().find(&mut state, &"d".into()).unwrap() {
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

    #[test]
    fn residuals() {
        // p0-----line 1-----p1
        //
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
                Expression::parse("d1 = sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap(),
                Expression::parse("d1 = 5", false).unwrap(),
                Expression::parse("d1 = 5", false).unwrap(), // should de-dupe
            ],
        )
        .unwrap();

        // distance formula
        // d                            = sqrt( (x2 - x1)^2 + (y2 - y1)^2 )
        // d^2                          = (x2 - x1)^2 + (y2 - y1)^2
        // d^2 - (y2 - y1)^2            = (x2 - x1)^2
        // sqrt(d^2 - (y2 - y1)^2)      =  x2 - x1
        // sqrt(d^2 - (y2 - y1)^2) + x1 =  x2
        assert_eq!(
            SubSolver::default().all_remaining_residuals(&mut state),
            HashMap::from([
                (
                    "x1".into(),
                    (
                        1,
                        Expression::parse("sqrt_pm(d1^2 - (y1 - y0)^2) + x0", false).unwrap()
                    )
                ),
                (
                    "y1".into(),
                    (
                        1,
                        Expression::parse("sqrt_pm(d1^2 - (x1 - x0)^2) + y0", false).unwrap()
                    )
                )
            ]),
        );

        assert_eq!(
            SubSolver::default().all_residuals(&mut state),
            vec![Expression::parse("d1 - (sqrt((x1-x0)^2 + (y1-y0)^2))", false).unwrap(),],
        );
        assert_eq!(
            SubSolver::default()
                .all_concrete_results(&mut state)
                .0
                .len(),
            3,
        );
        assert_eq!(
            SubSolver::default()
                .all_concrete_results(&mut state)
                .0
                .get(&"x0".into())
                .unwrap()
                .as_f64(),
            0.0,
        );
        assert_eq!(
            SubSolver::default()
                .all_concrete_results(&mut state)
                .0
                .get(&"y0".into())
                .unwrap()
                .as_f64(),
            1.0,
        );
        assert_eq!(
            SubSolver::default()
                .all_concrete_results(&mut state)
                .0
                .get(&"d1".into())
                .unwrap()
                .as_f64(),
            5.0,
        );

        // p0-----line 1-----p1-----line 2-----p2
        //
        // line 1 & 2 have fixed distance of 5
        // p0 is at (0, 1)
        // p2 is at (10, 1)

        state = SubSolverState::new(
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
                    "x2".into(),
                    Concrete::Rational(Rational::from_integer(5.into())),
                ),
                (
                    "y2".into(),
                    Concrete::Rational(Rational::from_integer(1.into())),
                ),
            ]),
            vec![
                Expression::parse("d1 = sqrt((x1-x0)^2 + (y1-y0)^2)", false).unwrap(),
                Expression::parse("d1 = 5", false).unwrap(),
                Expression::parse("d2 = sqrt((x2-x1)^2 + (y2-y1)^2)", false).unwrap(),
                Expression::parse("d2 = 5", false).unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(
            SubSolver::default().all_residuals(&mut state),
            vec![
                Expression::parse("d1 - 5", false).unwrap(),
                Expression::parse("d2 - 5", false).unwrap(),
                Expression::parse("d2 - (sqrt((x2-x1)^2 + (y2-y1)^2))", false).unwrap(),
                Expression::parse("d1 - (sqrt((x1-x0)^2 + (y1-y0)^2))", false).unwrap(),
            ],
        );

        // for (v, e) in SubSolver::default().all_remaining_residuals(&mut state).iter() {
        //     println!("{} = {}", v, e.1);
        // }
        assert_eq!(
            SubSolver::default().all_remaining_residuals(&mut state)[&"x1".into()]
                .1
                .evaluate(
                    &mut StaticResolver::new([
                        (
                            "d1".into(),
                            Concrete::Rational(Rational::new(5.into(), 1.into()))
                        ),
                        (
                            "d2".into(),
                            Concrete::Rational(Rational::new(5.into(), 1.into()))
                        ),
                        (
                            "y1".into(),
                            Concrete::Rational(Rational::new(1.into(), 1.into()))
                        ),
                        (
                            "x0".into(),
                            Concrete::Rational(Rational::new(0.into(), 1.into()))
                        ),
                        (
                            "y0".into(),
                            Concrete::Rational(Rational::new(1.into(), 1.into()))
                        ),
                        (
                            "x2".into(),
                            Concrete::Rational(Rational::new(10.into(), 1.into()))
                        ),
                        (
                            "y2".into(),
                            Concrete::Rational(Rational::new(1.into(), 1.into()))
                        ),
                    ]),
                    0
                )
                .unwrap()
                .as_f64(),
            10.0, // should equal (2 * x1) == 10
        );
        assert_eq!(
            SubSolver::default().all_remaining_residuals(&mut state)[&"y1".into()]
                .1
                .evaluate(
                    &mut StaticResolver::new([
                        (
                            "d1".into(),
                            Concrete::Rational(Rational::new(5.into(), 1.into()))
                        ),
                        (
                            "d2".into(),
                            Concrete::Rational(Rational::new(5.into(), 1.into()))
                        ),
                        (
                            "x1".into(),
                            Concrete::Rational(Rational::new(5.into(), 1.into()))
                        ),
                        (
                            "x0".into(),
                            Concrete::Rational(Rational::new(0.into(), 1.into()))
                        ),
                        (
                            "y0".into(),
                            Concrete::Rational(Rational::new(1.into(), 1.into()))
                        ),
                        (
                            "x2".into(),
                            Concrete::Rational(Rational::new(10.into(), 1.into()))
                        ),
                        (
                            "y2".into(),
                            Concrete::Rational(Rational::new(1.into(), 1.into()))
                        ),
                    ]),
                    0
                )
                .unwrap()
                .as_f64(),
            2.0, // (2 * y1) == 2
        );
    }
}
