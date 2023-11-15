//pub const MAX_EQ_ELEMENTS: usize = 16;
mod parser;
pub mod solve;

/// Algebraic unknown, identified by a name up to 12 characters long.
pub type Variable = heapless::String<12>;

/// Algebraic integer.
pub type Integer = num::bigint::BigInt;

/// Algebraic rational number.
pub type Rational = num::rational::Ratio<Integer>;

/// Finite value of some variable.
#[derive(Clone, Debug)]
pub enum Concrete {
    Rational(Rational),
    Float(f64),
}

impl Concrete {
    pub fn as_f64(&self) -> f64 {
        use num::ToPrimitive;
        match self {
            Concrete::Float(f) => *f,
            Concrete::Rational(r) => r.to_f64().unwrap(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveErr {
    UnknownVar(Variable),
    PowUnable(Rational),
    DivByZero,

    CannotSolve,
    NotImplementedOrWhatever,
}

pub trait Resolver {
    fn resolve_variable(&mut self, v: &Variable) -> Result<Concrete, ResolveErr>;
}

/// Equation element.
#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub enum Expression {
    /// Variable with identifier.
    Variable(Variable),
    /// Integer.
    Integer(Integer),
    /// Rational number, .1 is true if it should be printed as fraction.
    Rational(Rational, bool),

    /// Whether two expressions are equal.
    Equal(Box<Self>, Box<Self>),

    /// Negation of expression.
    Neg(Box<Self>),
    /// Absolute value of an expression.
    Abs(Box<Self>),
    // Sqrt of one expression, .1 is true if it can be either plus or minus.
    Sqrt(Box<Self>, bool),

    /// Sum of 2 expressions.
    Sum(Box<Self>, Box<Self>),
    /// Difference of 2 expressions.
    Difference(Box<Self>, Box<Self>),
    /// Product of 2 expressions.
    Product(Box<Self>, Box<Self>),
    /// Division of 2 expressions.
    Quotient(Box<Self>, Box<Self>),
    /// Power of one expression by another.
    Power(Box<Self>, Box<Self>),
    // Something(heapless::Vec<Box<Self>, MAX_EQ_ELEMENTS>),
}

/// An operation to apply when rearranging the equation
#[derive(PartialEq, Eq, Clone, Debug)]
enum ReverseOp {
    Multiply(Expression),
    Divide(Expression),
    Add(Expression),
    Sub(Expression),
    DivideUnder(Expression),
    Power(Expression),
    Sqrt,
}

impl Expression {
    pub fn walk(&self, cb: &mut impl FnMut(&Expression) -> bool) {
        if !cb(self) {
            return;
        }

        // recurse to sub-expressions
        match self {
            // binary
            Expression::Sum(a, b)
            | Expression::Difference(a, b)
            | Expression::Product(a, b)
            | Expression::Quotient(a, b)
            | Expression::Power(a, b)
            | Expression::Equal(a, b) => {
                a.walk(cb);
                b.walk(cb);
            }
            // unary
            Expression::Neg(a) | Expression::Sqrt(a, _) | Expression::Abs(a) => a.walk(cb),
            // no sub-expressions
            Expression::Integer(_) | Expression::Rational(_, _) | Expression::Variable(_) => {}
        }
    }

    pub fn cost(&self) -> usize {
        let mut cost = 0;
        self.walk(&mut |e| {
            match e {
                Expression::Sum(_, _) | Expression::Difference(_, _) | Expression::Neg(_) => {
                    cost += 2;
                }
                Expression::Product(_, _) => {
                    cost += 4;
                }
                Expression::Quotient(_, _) | Expression::Variable(_) => {
                    cost += 5;
                }
                Expression::Integer(_) | Expression::Rational(_, _) => {
                    cost += 1;
                }
                Expression::Power(_, _) | Expression::Abs(_) => {
                    cost += 10;
                }
                Expression::Sqrt(_, _) => {
                    cost += 25;
                }
                _ => {}
            };
            true
        });
        cost
    }

    pub fn evaluate<R: Resolver>(&self, r: &mut R) -> Result<Concrete, ResolveErr> {
        // TODO: support multiple results in return set
        match self {
            Expression::Sum(a, b) => match (a.evaluate(r)?, b.evaluate(r)?) {
                (Concrete::Rational(a), Concrete::Rational(b)) => Ok(Concrete::Rational(a + b)),
                (a, b) => Ok(Concrete::Float(a.as_f64() + b.as_f64())),
            },
            Expression::Difference(a, b) => match (a.evaluate(r)?, b.evaluate(r)?) {
                (Concrete::Rational(a), Concrete::Rational(b)) => Ok(Concrete::Rational(a - b)),
                (a, b) => Ok(Concrete::Float(a.as_f64() - b.as_f64())),
            },
            Expression::Product(a, b) => match (a.evaluate(r)?, b.evaluate(r)?) {
                (Concrete::Rational(a), Concrete::Rational(b)) => Ok(Concrete::Rational(a * b)),
                _ => todo!("{:?} * {:?}", a, b),
            },
            Expression::Quotient(a, b) => match (a.evaluate(r)?, b.evaluate(r)?) {
                (Concrete::Rational(a), Concrete::Rational(b)) => {
                    if b == Rational::from_integer(0.into()) {
                        Err(ResolveErr::DivByZero)
                    } else {
                        Ok(Concrete::Rational(a / b))
                    }
                }
                _ => todo!("{:?} / {:?}", a, b),
            },

            Expression::Neg(a) => match a.evaluate(r)? {
                Concrete::Rational(a) => Ok(Concrete::Rational(-a)),
                _ => todo!("-{:?}", a),
            },
            Expression::Abs(a) => match a.evaluate(r)? {
                Concrete::Rational(a) => {
                    use num::Signed;
                    Ok(Concrete::Rational(a.abs()))
                }
                a => Ok(Concrete::Float(a.as_f64().abs())),
            },
            Expression::Sqrt(a, _) => Ok(Concrete::Float(a.evaluate(r)?.as_f64().sqrt())),

            Expression::Power(a, b) => match (a.evaluate(r)?, b.evaluate(r)?) {
                (Concrete::Rational(a), Concrete::Rational(b)) => {
                    use num::ToPrimitive;
                    match b.to_i32() {
                        Some(b) => Ok(Concrete::Rational(a.pow(b))),
                        None => Err(ResolveErr::PowUnable(b)),
                    }
                }
                (Concrete::Float(a), Concrete::Rational(b)) => {
                    use num::ToPrimitive;
                    match b.to_i32() {
                        Some(b) => Ok(Concrete::Float(a.powi(b))),
                        None => Ok(Concrete::Float(a.powf(b.to_f64().unwrap()))),
                    }
                }
                (a, b) => Ok(Concrete::Float(a.as_f64().powf(b.as_f64()))),
            },

            Expression::Integer(i) => Ok(Concrete::Rational(Rational::from_integer(i.clone()))),
            Expression::Rational(r, _) => Ok(Concrete::Rational(r.clone())),
            Expression::Variable(v) => r.resolve_variable(v),
            _ => todo!("{:?}", self),
        }
    }

    fn is_coefficient(&self) -> bool {
        match self {
            Expression::Integer(_) => true,
            Expression::Rational(_, _) => true,
            _ => false,
        }
    }

    pub fn simplify(&mut self) {
        // recurse to sub-expressions
        match self {
            // binary
            Expression::Sum(a, b)
            | Expression::Difference(a, b)
            | Expression::Product(a, b)
            | Expression::Quotient(a, b)
            | Expression::Power(a, b)
            | Expression::Equal(a, b) => {
                a.simplify();
                b.simplify();
            }
            // unary
            Expression::Neg(a) | Expression::Sqrt(a, _) | Expression::Abs(a) => a.simplify(),
            // no sub-expressions
            Expression::Integer(_) | Expression::Rational(_, _) | Expression::Variable(_) => {}
        }

        // handle any simplifications we can do at our end
        self.simplify_self();
    }

    fn normalize_2x(&mut self) {
        // Negation of a constant
        if let Expression::Neg(a) = self {
            match a.as_ref() {
                Expression::Integer(a) => {
                    *self = Expression::Integer(a * -1);
                }
                Expression::Rational(a, as_fraction) => {
                    *self =
                        Expression::Rational(a * Rational::from_integer((-1).into()), *as_fraction);
                }
                _ => {}
            }
        }

        // Use integer representation when possible
        if let Expression::Rational(a, _) = self {
            if a.is_integer() {
                *self = Expression::Integer(a.numer().clone());
            }
        }

        // Products should put coefficients as first operand.
        if let Expression::Product(a, b) = self {
            if !a.is_coefficient() && b.is_coefficient() {
                std::mem::swap(b, a);
            }
        }

        // Negation of a product with a coefficient
        if let Expression::Neg(a) = self {
            if let Expression::Product(a, b) = a.as_ref() {
                match a.as_ref() {
                    Expression::Integer(a) => {
                        *self =
                            Expression::Product(Box::new(Expression::Integer(a * -1)), b.clone());
                    }
                    Expression::Rational(a, as_fraction) => {
                        *self = Expression::Product(
                            Box::new(Expression::Rational(
                                a * Rational::from_integer(Integer::from(-1)),
                                *as_fraction,
                            )),
                            b.clone(),
                        );
                    }
                    _ => {}
                }
            }
        }

        // Abs of a negation
        if let Expression::Abs(a) = self {
            if let Expression::Neg(b) = a.as_ref() {
                let temp = b.to_owned();
                let _ = std::mem::replace(&mut *a, temp);
            }
        }
    }

    fn normalize(&mut self) {
        self.normalize_2x();

        // Product with a rational where numerator is 1 => Quotient(term / denom)
        if let Expression::Product(a, b) = self {
            match (a.as_ref(), b.as_ref()) {
                (Expression::Rational(r, _), _) => {
                    if r.numer() == &Integer::from(1) {
                        *self = Expression::Quotient(
                            b.clone(),
                            Box::new(Expression::Integer(r.denom().clone())),
                        );
                    }
                }
                (_, Expression::Rational(r, _)) => {
                    if r.numer() == &Integer::from(1) {
                        *self = Expression::Quotient(
                            a.clone(),
                            Box::new(Expression::Integer(r.denom().clone())),
                        );
                    }
                }
                _ => {}
            }
        }

        // Sum with an operand of 0.
        if let Expression::Sum(a, b) = self {
            match (a.as_ref(), b.as_ref()) {
                (Expression::Integer(a), _) => {
                    if *a == 0.into() {
                        *self = *b.clone();
                    }
                }
                (_, Expression::Integer(b)) => {
                    if *b == 0.into() {
                        *self = *a.clone();
                    }
                }
                _ => {}
            }
        }
        // Sum with a negative: converted to outer neg or subtraction.
        if let Expression::Sum(a, b) = self {
            if let Expression::Neg(a) = a.as_ref() {
                if let Expression::Neg(b) = b.as_ref() {
                    *self = Expression::Neg(Box::new(Expression::Sum(a.clone(), b.clone())));
                } else {
                    *self = Expression::Difference(b.clone(), a.clone());
                }
            }
        }

        // Difference with an operand of 0.
        if let Expression::Difference(a, b) = self {
            match (a.as_ref(), b.as_ref()) {
                (Expression::Integer(a), _) => {
                    if *a == 0.into() {
                        *self = Expression::Neg(b.clone());
                    }
                }
                (_, Expression::Integer(b)) => {
                    if *b == 0.into() {
                        *self = *a.clone();
                    }
                }
                _ => {}
            }
        }
        // Difference with one negative: convert to negation of a sum
        if let Expression::Difference(a, b) = self {
            if let Expression::Neg(a) = a.as_ref() {
                let op2_is_neg = matches!(b.as_ref(), Expression::Neg(_));
                if !op2_is_neg {
                    *self = Expression::Neg(Box::new(Expression::Sum(a.clone(), b.clone())));
                }
            }
        }

        // Multiply with an operand of 0 or 1 or -1.
        // Multiplication with -1 is transformed to a Neg.
        if let Expression::Product(a, b) = self {
            match (a.as_ref(), b.as_ref()) {
                (Expression::Integer(a), _) => {
                    if *a == 0.into() {
                        *self = Expression::Integer(0.into());
                    } else if *a == 1.into() {
                        *self = *b.clone();
                    } else if *a == (-1).into() {
                        *self = Expression::Neg(b.clone());
                    }
                }
                (_, Expression::Integer(b)) => {
                    if *b == 0.into() {
                        *self = Expression::Integer(0.into());
                    } else if *b == 1.into() {
                        *self = *a.clone();
                    } else if *b == (-1).into() {
                        *self = Expression::Neg(a.clone());
                    }
                }
                _ => {}
            }
        }

        // Power-of with a power of 0, 1, or -1.
        if let Expression::Power(a, b) = self {
            match (a.as_ref(), b.as_ref()) {
                (_, Expression::Integer(b)) => {
                    if *b == 0.into() {
                        *self = Expression::Integer(1.into());
                    } else if *b == 1.into() {
                        *self = *a.clone();
                    } else if *b == (-1).into() {
                        *self = Expression::Quotient(
                            Box::new(Expression::Integer(1.into())),
                            Box::new(*a.clone()),
                        );
                    }
                }
                _ => {}
            }
        }

        // Divide with an operand of 0, 1, or -1.
        if let Expression::Quotient(a, b) = self {
            match (a.as_ref(), b.as_ref()) {
                (Expression::Integer(a), _) => {
                    if *a == 0.into() {
                        *self = Expression::Integer(0.into());
                    }
                }
                (_, Expression::Integer(b)) => {
                    if *b == 1.into() {
                        *self = *a.clone();
                    } else if *b == (-1).into() {
                        *self = Expression::Neg(Box::new(*a.clone()));
                    }
                }
                _ => {}
            }
        }

        if let Expression::Sqrt(a, _) = self {
            match a.as_ref() {
                // Sqrt of square: simplify to abs(term)
                Expression::Power(a, b) => {
                    if let Expression::Integer(b) = b.as_ref() {
                        if *b == 2.into() {
                            *self = Expression::Abs(a.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        self.normalize_2x();
    }

    fn simplify_self(&mut self) {
        self.normalize();

        match self {
            Expression::Quotient(a, b) => match (a.as_ref(), b.as_ref()) {
                // Division of two integers means a rational, possibly folding
                // into constant integer
                (Expression::Integer(a), Expression::Integer(b)) => {
                    if a == b {
                        *self = Expression::Integer(1.into());
                    } else {
                        let r = Rational::new(a.clone(), b.clone());
                        if r.is_integer() {
                            *self = Expression::Integer(r.numer().clone());
                        } else {
                            *self = Expression::Rational(r, true);
                        }
                    }
                }
                // Constant folding: Division of two rationals
                (Expression::Rational(a, as_fraction), Expression::Rational(b, _)) => {
                    if a == b {
                        *self = Expression::Integer(1.into());
                    } else {
                        *self = Expression::Rational(a / b, *as_fraction);
                    }
                }
                // Constant folding: Division of rational by integer
                (Expression::Rational(a, as_fraction), Expression::Integer(b)) => {
                    *self = Expression::Rational(a / b, *as_fraction);
                }
                // Constant folding: Division of integer by rational
                (Expression::Integer(a), Expression::Rational(b, as_fraction)) => {
                    *self =
                        Expression::Rational(Rational::from_integer(a.clone()) / b, *as_fraction);
                }
                _ => {
                    // Divison by two identical terms is a 1.
                    if a == b {
                        *self = Expression::Integer(1.into());
                    }
                }
            },

            Expression::Sum(a, b) => match (a.as_ref(), b.as_ref()) {
                // Constant folding: integer addition
                (Expression::Integer(a), Expression::Integer(b)) => {
                    *self = Expression::Integer(a + b);
                }
                // Constant folding: rational addition
                (Expression::Rational(a, as_fraction), Expression::Rational(b, _)) => {
                    *self = Expression::Rational(a + b, *as_fraction);
                }
                // Constant folding: mixed rational/integer addition
                (Expression::Rational(a, as_fraction), Expression::Integer(b))
                | (Expression::Integer(b), Expression::Rational(a, as_fraction)) => {
                    *self = Expression::Rational(a + b, *as_fraction);
                }
                // ax + bx = (a+b)x
                (Expression::Product(a, x1), Expression::Product(b, x2)) => {
                    if let (Expression::Integer(a), Expression::Integer(b)) =
                        (a.as_ref(), b.as_ref())
                    {
                        if x1 == x2 {
                            *self = Expression::Product(
                                Box::new(Expression::Integer(a + b)),
                                x1.clone(),
                            );
                        }
                    }
                }

                _ => {
                    // Sum of two identical terms is 2*term.
                    if a == b {
                        *self =
                            Expression::Product(Box::new(Expression::Integer(2.into())), a.clone());
                    }
                }
            },

            Expression::Difference(a, b) => match (a.as_ref(), b.as_ref()) {
                // Constant folding: integer subtraction
                (Expression::Integer(a), Expression::Integer(b)) => {
                    *self = Expression::Integer(a - b);
                }
                // Constant folding: rational subtraction
                (Expression::Rational(a, as_fraction), Expression::Rational(b, _)) => {
                    *self = Expression::Rational(a - b, *as_fraction);
                }
                // Constant folding: Difference of rational with integer
                (Expression::Rational(a, as_fraction), Expression::Integer(b)) => {
                    *self = Expression::Rational(a - b, *as_fraction);
                }
                // Constant folding: Difference of integer with rational
                (Expression::Integer(a), Expression::Rational(b, as_fraction)) => {
                    *self =
                        Expression::Rational(Rational::from_integer(a.clone()) - b, *as_fraction);
                }
                // ax - bx = (a-b)x
                (Expression::Product(a, x1), Expression::Product(b, x2)) => {
                    if let (Expression::Integer(a), Expression::Integer(b)) =
                        (a.as_ref(), b.as_ref())
                    {
                        if x1 == x2 {
                            *self = Expression::Product(
                                Box::new(Expression::Integer(a - b)),
                                x1.clone(),
                            );
                        }
                    }
                }

                _ => {
                    // Difference of two identical terms is zero.
                    if a == b {
                        *self = Expression::Integer(0.into());
                    } else
                    // a--a = 2a
                    if &Expression::Neg(a.clone()) == b.as_ref() {
                        *self = Expression::Product(
                            Box::new(Expression::Integer(2.into())),
                            a.to_owned(),
                        );
                    }
                }
            },

            Expression::Product(a, b) => match (a.as_ref(), b.as_ref()) {
                // Constant folding: integer multiplication
                (Expression::Integer(a), Expression::Integer(b)) => {
                    *self = Expression::Integer(a * b);
                }
                // Constant folding: rational multiplication
                (Expression::Rational(a, as_fraction), Expression::Rational(b, _)) => {
                    *self = Expression::Rational(a * b, *as_fraction);
                }
                // Constant folding: mixed rational/integer multiplication
                (Expression::Rational(a, as_fraction), Expression::Integer(b))
                | (Expression::Integer(b), Expression::Rational(a, as_fraction)) => {
                    *self = Expression::Rational(a * b, *as_fraction);
                }
                _ => {
                    // Multiplication of identical terms is pow(a, 2)
                    if a == b {
                        *self =
                            Expression::Power(a.clone(), Box::new(Expression::Integer(2.into())));
                    }
                }
            },

            Expression::Sqrt(a, _) => match a.as_ref() {
                // Constant folding: integer sqrt
                // TODO: consult/support add/minus
                Expression::Integer(a) => {
                    *self = Expression::Integer(a.sqrt());
                }
                _ => {}
            },

            Expression::Power(a, b) => match (a.as_ref(), b.as_ref()) {
                // Constant folding: integer base, common powers
                (Expression::Integer(a), Expression::Integer(b)) => {
                    if *b == 2.into() {
                        *self = Expression::Integer(a * a);
                    } else if *b == 3.into() {
                        *self = Expression::Integer(a * a * a);
                    } else if *b == 4.into() {
                        *self = Expression::Integer(a * a * a * a);
                    }
                }
                // Constant folding: rational base, common powers
                (Expression::Rational(a, as_fraction), Expression::Integer(b)) => {
                    if *b == 2.into() {
                        *self = Expression::Rational(a * a, *as_fraction);
                    } else if *b == 3.into() {
                        *self = Expression::Rational(a * a * a, *as_fraction);
                    } else if *b == 4.into() {
                        *self = Expression::Rational(a * a * a * a, *as_fraction);
                    }
                }
                _ => {}
            },
            _ => {}
        }

        self.normalize();
    }

    pub fn make_subject(&self, var: &Expression) -> Result<Self, ()> {
        if let Expression::Equal(lhs, rhs) = self {
            if var == &**rhs {
                return Ok(Expression::Equal(Box::new(var.clone()), lhs.clone()));
            }

            if let Some(reverse_ops) = rhs.raise_for(var)? {
                let mut lhs = lhs.clone().apply(reverse_ops);
                lhs.simplify();
                return Ok(Expression::Equal(Box::new(var.clone()), Box::new(lhs)));
            }
            if let Some(reverse_ops) = lhs.raise_for(var)? {
                let mut rhs = rhs.clone().apply(reverse_ops);
                rhs.simplify();
                return Ok(Expression::Equal(Box::new(var.clone()), Box::new(rhs)));
            }

            Err(())
        } else {
            Err(())
        }
    }

    // WIP: idea
    //
    // // factor_for succeeds if it was able to move the want expression to
    // // one of the operands. True is returned as the Ok value if it was
    // // the second operand, false for the first operand.
    // fn factor_for(&mut self, want: &Expression) -> Result<bool, ()> {
    //     match self {
    //         Expression::Sum(a, b) => {}
    //         _ => {}
    //     }

    //     Err(())
    // }

    /// Recursively computes the set of operations needed to make a term the
    /// subject of an equation.
    fn raise_for(&self, want: &Expression) -> Result<Option<Vec<ReverseOp>>, ()> {
        if self == want {
            return Ok(Some(vec![]));
        }

        match self {
            Expression::Sum(a, b) => {
                // TODO: handle case where want expr is in both terms.
                match a.raise_for(want)? {
                    Some(mut ops) => {
                        ops.push(ReverseOp::Sub((**b).clone()));
                        Ok(Some(ops))
                    }
                    None => match b.raise_for(want)? {
                        Some(mut ops) => {
                            ops.push(ReverseOp::Sub((**a).clone()));
                            Ok(Some(ops))
                        }
                        None => Ok(None),
                    },
                }
            }
            Expression::Difference(a, b) => {
                // TODO: handle case where want expr is in both operands.
                match a.raise_for(want)? {
                    Some(mut ops) => {
                        ops.push(ReverseOp::Add((**b).clone()));
                        Ok(Some(ops))
                    }
                    None => match b.raise_for(want)? {
                        Some(mut ops) => {
                            ops.push(ReverseOp::Add((**a).clone()));
                            ops.push(ReverseOp::Multiply(Expression::Integer((-1).into())));
                            Ok(Some(ops))
                        }
                        None => Ok(None),
                    },
                }
            }
            Expression::Product(a, b) => {
                // TODO: handle case where want expr is in both terms.
                match a.raise_for(want)? {
                    Some(mut ops) => {
                        ops.push(ReverseOp::Divide((**b).clone()));
                        Ok(Some(ops))
                    }
                    None => match b.raise_for(want)? {
                        Some(mut ops) => {
                            ops.push(ReverseOp::Divide((**a).clone()));
                            Ok(Some(ops))
                        }
                        None => Ok(None),
                    },
                }
            }
            Expression::Quotient(a, b) => {
                // TODO: handle case where want expr is in both numerator and denominator.
                match a.raise_for(want)? {
                    Some(mut ops) => {
                        ops.push(ReverseOp::Multiply((**b).clone()));
                        Ok(Some(ops))
                    }
                    None => match b.raise_for(want)? {
                        Some(mut ops) => {
                            ops.push(ReverseOp::DivideUnder((**a).clone()));
                            Ok(Some(ops))
                        }
                        None => Ok(None),
                    },
                }
            }
            Expression::Power(a, b) => {
                if let Expression::Integer(pow) = b.as_ref() {
                    if pow == &Integer::from(2) {
                        match a.raise_for(want)? {
                            Some(mut ops) => {
                                ops.push(ReverseOp::Sqrt);
                                Ok(Some(ops))
                            }
                            None => Ok(None),
                        }
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }

            Expression::Neg(a) => match a.raise_for(want)? {
                Some(mut ops) => {
                    ops.push(ReverseOp::Multiply(Expression::Integer((-1).into())));
                    Ok(Some(ops))
                }
                None => Ok(None),
            },
            // TODO: support add/minus
            Expression::Sqrt(a, _) => match a.raise_for(want)? {
                Some(mut ops) => {
                    ops.push(ReverseOp::Power(Expression::Integer(2.into())));
                    Ok(Some(ops))
                }
                None => Ok(None),
            },

            Expression::Integer(_) | Expression::Rational(_, _) | Expression::Variable(_) => {
                Ok(None)
            }

            _ => todo!(),
        }
    }

    fn apply(mut self: Self, ops: Vec<ReverseOp>) -> Self {
        for op in ops.into_iter().rev() {
            match op {
                ReverseOp::Multiply(exp) => {
                    self = Expression::Product(Box::new(self), Box::new(exp.clone()));
                }
                ReverseOp::Divide(exp) => {
                    self = Expression::Quotient(Box::new(self), Box::new(exp.clone()));
                }
                ReverseOp::DivideUnder(exp) => {
                    self = Expression::Quotient(Box::new(exp.clone()), Box::new(self));
                }
                ReverseOp::Add(exp) => {
                    self = Expression::Sum(Box::new(self), Box::new(exp.clone()));
                }
                ReverseOp::Sub(exp) => {
                    self = Expression::Difference(Box::new(self), Box::new(exp.clone()));
                }
                ReverseOp::Power(exp) => {
                    self = Expression::Power(Box::new(self), Box::new(exp.clone()));
                }
                ReverseOp::Sqrt => {
                    self = Expression::Sqrt(Box::new(self), true);
                }
            }
        }

        self
    }

    pub fn parse<'a>(
        expression: &'a str,
        simplify: bool,
    ) -> Result<Self, Vec<chumsky::prelude::EmptyErr>> {
        use chumsky::Parser;
        match parser::parse_expr().parse(expression).into_result() {
            Ok(mut exp) => {
                if simplify {
                    exp.simplify();
                }
                Ok(exp)
            }
            Err(e) => Err(e),
        }
    }
}

fn decimal_representation(x: &Rational) -> Option<(Integer, usize)> {
    let mut denom = x.denom().clone();

    // See: https://cs.stackexchange.com/questions/124673/algorithm-turining-a-fraction-into-a-decimal-expansion-string
    let [power_of_2, power_of_5] = [2, 5].map(|n| {
        let mut power = 0;

        while (denom.clone() % Integer::from(n)).is_zero() {
            denom /= n;
            power += 1;
        }

        power
    });

    use num::{One, Zero};
    if denom.is_one() {
        Some((
            x.numer()
                * if power_of_2 < power_of_5 {
                    Integer::from(2).pow(power_of_5 - power_of_2)
                } else {
                    Integer::from(5).pow(power_of_2 - power_of_5)
                },
            std::cmp::max(power_of_2, power_of_5) as usize,
        ))
    } else {
        None
    }
}

use std::fmt::{Display, Formatter};
impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use num::ToPrimitive;

        match self {
            Expression::Neg(e) => write!(f, "-{}", e),
            Expression::Abs(e) => write!(f, "abs({})", e),
            Expression::Sqrt(a, pm) => match pm {
                false => write!(f, "sqrt({})", a),
                true => write!(f, "sqrt_pm({})", a),
            },

            Expression::Variable(v) => write!(f, "{}", v),
            Expression::Integer(i) => write!(f, "{}", i),
            Expression::Rational(r, as_rational) => match as_rational {
                true => write!(f, "({}/{})", r.numer(), r.denom()),
                false => {
                    if let Some((mantissa, idx)) = decimal_representation(r) {
                        let mut out = mantissa.abs().to_string();

                        if idx > 0 {
                            if idx > out.len() - 1 {
                                // Left-pad the string with enough zeros to be able
                                // to insert the decimal separator at the indicated position.
                                out = format!("{}{}", "0".repeat(idx - (out.len() - 1)), out,);
                            }

                            out.insert(out.len() - idx, '.');
                        }

                        use num::Signed;
                        write!(f, "{}{}", if r.is_negative() { "-" } else { "" }, out)
                    } else if let (Some(n), Some(d)) = (r.numer().to_f64(), r.denom().to_f64()) {
                        write!(f, "{}", n / d)
                    } else {
                        write!(f, "({}/{})", r.numer(), r.denom())
                    }
                }
            },

            Expression::Equal(a, b) => write!(f, "{} = {}", a, b),
            Expression::Sum(a, b) => write!(f, "({} + {})", a, b),
            Expression::Difference(a, b) => write!(f, "({} - {})", a, b),
            Expression::Quotient(a, b) => write!(f, "({} / {})", a, b),
            Expression::Product(a, b) => match (a.as_ref(), b.as_ref()) {
                (Expression::Integer(a), Expression::Variable(v)) => write!(f, "{}{}", a, v),
                _ => write!(f, "({} * {})", a, b),
            },
            Expression::Power(a, b) => write!(f, "({})^{}", a, b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplifications() {
        assert_eq!(
            Expression::parse("1 + 2", true),
            Ok(Expression::Integer(3.into()))
        );
        assert_eq!(
            Expression::parse("1 + 2", false),
            Ok(Expression::Sum(
                Box::new(Expression::Integer(1.into())),
                Box::new(Expression::Integer(2.into())),
            ))
        );
        assert_eq!(
            Expression::parse("1/2 + 1/2", true),
            Ok(Expression::Integer(1.into()))
        );

        assert_eq!(
            Expression::parse("4/2", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("2+0", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("0+2", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("2*0", true),
            Ok(Expression::Integer(0.into()))
        );
        assert_eq!(
            Expression::parse("0*2", true),
            Ok(Expression::Integer(0.into()))
        );
        assert_eq!(
            Expression::parse("2*1", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("1*2", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("5-0", true),
            Ok(Expression::Integer(5.into()))
        );
        assert_eq!(
            Expression::parse("0-5", true),
            Ok(Expression::Integer((-5).into()))
        );
        assert_eq!(
            Expression::parse("3/1", true),
            Ok(Expression::Integer(3.into()))
        );
        assert_eq!(
            Expression::parse("0/5", true),
            Ok(Expression::Integer(0.into()))
        );
        assert_eq!(
            Expression::parse("a/a", true),
            Ok(Expression::Integer(1.into()))
        );
        assert_eq!(
            Expression::parse("a-a", true),
            Ok(Expression::Integer(0.into()))
        );
        assert_eq!(
            Expression::parse("a+a", true),
            Ok(Expression::Product(
                Box::new(Expression::Integer(2.into())),
                Box::new(Expression::Variable("a".into())),
            ))
        );

        assert_eq!(
            Expression::parse("sqrt(4)", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("sqrt(3.5 + 1/2)", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("abs(-a)", true),
            Ok(Expression::Abs(Box::new(Expression::Variable("a".into())),))
        );
        assert_eq!(
            Expression::parse("sqrt(a^2)", true),
            Ok(Expression::Abs(Box::new(Expression::Variable("a".into())),))
        );

        assert_eq!(
            Expression::parse("a^1", true),
            Ok(Expression::Variable("a".into()))
        );
        assert_eq!(
            Expression::parse("a^0", true),
            Ok(Expression::Integer(1.into()))
        );
        assert_eq!(
            Expression::parse("a^-1", true),
            Ok(Expression::Quotient(
                Box::new(Expression::Integer(1.into())),
                Box::new(Expression::Variable("a".into())),
            ))
        );

        assert_eq!(
            Expression::parse("1 - 2", true),
            Ok(Expression::Integer((-1).into()))
        );
        assert_eq!(
            Expression::parse("1 - 2", false),
            Ok(Expression::Difference(
                Box::new(Expression::Integer(1.into())),
                Box::new(Expression::Integer(2.into())),
            ))
        );
        assert_eq!(
            Expression::parse("1 * 2", true),
            Ok(Expression::Integer(2.into()))
        );
        assert_eq!(
            Expression::parse("1 * 2", false),
            Ok(Expression::Product(
                Box::new(Expression::Integer(1.into())),
                Box::new(Expression::Integer(2.into())),
            ))
        );

        assert_eq!(
            Expression::parse("-x + -y", true),
            Ok(Expression::Neg(Box::new(Expression::Sum(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Variable("y".into())),
            )),))
        );

        assert_eq!(
            Expression::parse("-2x", true),
            Ok(Expression::Product(
                Box::new(Expression::Integer((-2).into())),
                Box::new(Expression::Variable("x".into())),
            ))
        );
    }

    #[test]
    fn simplifications_complex() {
        assert_eq!(
            Expression::parse("12 + 1 / 3 = 37/3", true),
            Ok(Expression::Equal(
                Box::new(Expression::Rational(
                    Rational::new(37.into(), 3.into()),
                    true,
                )),
                Box::new(Expression::Rational(
                    Rational::new(37.into(), 3.into()),
                    true
                ))
            ))
        );
        assert_eq!(
            Expression::parse("12 + 1 / 3 = 13", false),
            Ok(Expression::Equal(
                Box::new(Expression::Sum(
                    Box::new(Expression::Integer(12.into())),
                    Box::new(Expression::Quotient(
                        Box::new(Expression::Integer(1.into())),
                        Box::new(Expression::Integer(3.into())),
                    )),
                )),
                Box::new(Expression::Integer(13.into()))
            ))
        );
        assert_eq!(
            Expression::parse("y = (x - 1)/2", false),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("y".into())),
                Box::new(Expression::Quotient(
                    Box::new(Expression::Difference(
                        Box::new(Expression::Variable("x".into())),
                        Box::new(Expression::Integer(1.into())),
                    )),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );

        assert_eq!(
            Expression::parse("(-a)-a", true),
            Ok(Expression::Neg(Box::new(Expression::Sum(
                Box::new(Expression::Variable("a".into())),
                Box::new(Expression::Variable("a".into())),
            ))))
        );
        assert_eq!(
            Expression::parse("a--a", true),
            Ok(Expression::Product(
                Box::new(Expression::Integer(2.into())),
                Box::new(Expression::Variable("a".into())),
            ))
        );
        assert_eq!(
            Expression::parse("(-a)--a", true),
            Ok(Expression::Integer(0.into()))
        );
        assert_eq!(
            Expression::parse("a/-1", true),
            Ok(Expression::Neg(Box::new(Expression::Variable("a".into()))))
        );

        assert_eq!(
            Expression::parse("1/2 + 1", true),
            Ok(Expression::Rational(
                Rational::new(3.into(), 2.into()),
                true
            )),
        );
        assert_eq!(
            Expression::parse("5 - 2^2", true),
            Ok(Expression::Integer(1.into()))
        );
        assert_eq!(
            Expression::parse("(1/3) ^ 2", true),
            Ok(Expression::Rational(
                Rational::new(1.into(), 9.into()),
                true
            )),
        );
        assert_eq!(
            Expression::parse("1/2 * 2", true),
            Ok(Expression::Integer(1.into()))
        );
        assert_eq!(
            Expression::parse("1/2 / 2", true),
            Ok(Expression::Rational(
                Rational::new(1.into(), 4.into()),
                true
            )),
        );
        assert_eq!(
            Expression::parse("1 - 1/2", true),
            Ok(Expression::Rational(
                Rational::new(1.into(), 2.into()),
                true
            )),
        );

        assert_eq!(
            Expression::parse("2x + 5x", true),
            Ok(Expression::Product(
                Box::new(Expression::Integer(7.into())),
                Box::new(Expression::Variable("x".into())),
            ))
        );
        assert_eq!(
            Expression::parse("5x -- 2x", true),
            Ok(Expression::Product(
                Box::new(Expression::Integer(7.into())),
                Box::new(Expression::Variable("x".into())),
            ))
        );

        // TODO: support factoring rationals and across types
        // assert_eq!(
        //     Expression::parse("(3/2 * x) + (2/5 * x)", true),
        //     Ok(Expression::Product(
        //         Box::new(Expression::Rational(
        //             Rational::new(1.into(), 2.into()),
        //             true
        //         )),
        //         Box::new(Expression::Variable("x".into())),
        //     ))
        // );
        // assert_eq!(
        //     Expression::parse("(3/2 * x) - (2/5 * x)", true),
        //     Ok(Expression::Product(
        //         Box::new(Expression::Rational(
        //             Rational::new(1.into(), 2.into()),
        //             true
        //         )),
        //         Box::new(Expression::Variable("x".into())),
        //     ))
        // );
    }

    #[test]
    fn make_subject() {
        assert_eq!(
            Expression::parse("y - 1 = x", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Difference(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(1.into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = -x", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Neg(Box::new(Expression::Variable("y".into())),)),
            ))
        );
        assert_eq!(
            Expression::parse("y = x + 2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Difference(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = x - 2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Sum(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = 2 - x", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Difference(
                    Box::new(Expression::Integer(2.into())),
                    Box::new(Expression::Variable("y".into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("2 - x = y", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Difference(
                    Box::new(Expression::Integer(2.into())),
                    Box::new(Expression::Variable("y".into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = x^2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Sqrt(
                    Box::new(Expression::Variable("y".into())),
                    true
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = sqrt(x)", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Power(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );

        assert_eq!(
            Expression::parse("y = 2 + x", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Difference(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = x + 2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Difference(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );

        assert_eq!(
            Expression::parse("y = x / 2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Product(
                    Box::new(Expression::Integer(2.into())),
                    Box::new(Expression::Variable("y".into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = 2 / x", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Quotient(
                    Box::new(Expression::Integer(2.into())),
                    Box::new(Expression::Variable("y".into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("3 / x = y", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Quotient(
                    Box::new(Expression::Integer(3.into())),
                    Box::new(Expression::Variable("y".into())),
                )),
            ))
        );

        assert_eq!(
            Expression::parse("y = x * 2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Quotient(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = 2 * x", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Quotient(
                    Box::new(Expression::Variable("y".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
            ))
        );

        assert_eq!(
            Expression::parse("y = 2 * (x + 3)", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Difference(
                    Box::new(Expression::Quotient(
                        Box::new(Expression::Variable("y".into())),
                        Box::new(Expression::Integer(2.into())),
                    )),
                    Box::new(Expression::Integer(3.into())),
                )),
            ))
        );

        assert_eq!(
            Expression::parse("y - 1 = x * 2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Quotient(
                    Box::new(Expression::Difference(
                        Box::new(Expression::Variable("y".into())),
                        Box::new(Expression::Integer(1.into())),
                    )),
                    Box::new(Expression::Integer(2.into()))
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = (x - 1)/2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Sum(
                    Box::new(Expression::Product(
                        Box::new(Expression::Integer(2.into())),
                        Box::new(Expression::Variable("y".into())),
                    )),
                    Box::new(Expression::Integer(1.into())),
                )),
            ))
        );

        assert_eq!(
            Expression::parse("x + y + z = 0", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Neg(Box::new(Expression::Sum(
                    Box::new(Expression::Variable("z".into())),
                    Box::new(Expression::Variable("y".into())),
                )),)),
            ))
        );

        assert_eq!(
            Expression::parse("y = (2x + 3x)/2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Quotient(
                    Box::new(Expression::Product(
                        Box::new(Expression::Integer(2.into())),
                        Box::new(Expression::Variable("y".into())),
                    )),
                    Box::new(Expression::Integer(5.into())),
                )),
            ))
        );
        assert_eq!(
            Expression::parse("y = (2x - 3x)/2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x".into())),
                Box::new(Expression::Product(
                    Box::new(Expression::Integer((-2).into())),
                    Box::new(Expression::Variable("y".into())),
                )),
            ))
        );

        // distance formula
        // d                            = sqrt( (x2 - x1)^2 + (y2 - y1)^2 )
        // d^2                          = (x2 - x1)^2 + (y2 - y1)^2
        // d^2 - (y2 - y1)^2            = (x2 - x1)^2
        // sqrt(d^2 - (y2 - y1)^2)      =  x2 - x1
        // sqrt(d^2 - (y2 - y1)^2) + x1 =  x2
        assert_eq!(
            Expression::parse("d = sqrt( (x2 - x1)^2 + (y2 - y1)^2 )", true)
                .unwrap()
                .make_subject(&Expression::Variable("x2".into())),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("x2".into())),
                Box::new(Expression::Sum(
                    Box::new(Expression::Sqrt(
                        Box::new(Expression::Difference(
                            Box::new(Expression::Power(
                                Box::new(Expression::Variable("d".into())),
                                Box::new(Expression::Integer(2.into())),
                            )),
                            Box::new(Expression::Power(
                                Box::new(Expression::Difference(
                                    Box::new(Expression::Variable("y2".into())),
                                    Box::new(Expression::Variable("y1".into())),
                                )),
                                Box::new(Expression::Integer(2.into())),
                            )),
                        ),),
                        true
                    )),
                    Box::new(Expression::Variable("x1".into())),
                )),
            ))
        );

        // circle formula
        assert_eq!(
            Expression::parse("r^2 = (x-h)^2 + (y-k)^2", true)
                .unwrap()
                .make_subject(&Expression::Variable("r".into())),
            Ok(Expression::parse("r = sqrt_pm((x-h)^2 + (y-k)^2)", true).unwrap()),
        );
        assert_eq!(
            Expression::parse("r^2 = (x-h)^2 + (y-k)^2", true)
                .unwrap()
                .make_subject(&Expression::Variable("x".into())),
            Ok(Expression::parse("x = sqrt_pm(r^2 - (y-k)^2) + h", true).unwrap()),
        );
    }
}
