use super::*;
use chumsky::prelude::*;

pub(super) fn parse_expr<'a>() -> impl Parser<'a, &'a str, Expression> {
    let ident = text::ident().padded();

    let expr = recursive(|expr| {
        let int = text::int(10).map(|s: &str| Expression::Integer(s.parse().unwrap()));

        let number = text::int(10)
            .then(just('.').then(text::int(10).slice().clone()))
            .map_slice(|s: &str| {
                let mut spl = s.split(".");
                let integer: Integer = spl.next().unwrap().parse().unwrap();
                let frac_str = spl.next().unwrap();
                let frac: Integer = frac_str.parse().unwrap();
                let num = Rational::new(integer, 1.into())
                    + Rational::new(frac, (10 * frac_str.len()).into());
                Expression::Rational(num, false)
            });

        let var_with_coeff = text::int(10)
            .then(text::ident())
            .map(|(coeff, var): (&str, &str)| {
                Expression::Product(
                    Box::new(Expression::Integer(coeff.parse().unwrap())),
                    Box::new(Expression::Variable(var.into())),
                )
            });

        let sqrt = text::keyword("sqrt")
            .then(expr.clone().delimited_by(just('('), just(')')))
            .map(|(_, e)| Expression::Sqrt(Box::new(e), false));
        let sqrt_pm = text::keyword("sqrt_pm")
            .then(expr.clone().delimited_by(just('('), just(')')))
            .map(|(_, e)| Expression::Sqrt(Box::new(e), true));
        let abs = text::keyword("abs")
            .then(expr.clone().delimited_by(just('('), just(')')))
            .map(|(_, e)| Expression::Abs(Box::new(e)));

        let atom = number
            .or(var_with_coeff)
            .or(int)
            .or(sqrt)
            .or(sqrt_pm)
            .or(abs)
            .or(expr.delimited_by(just('('), just(')')))
            .or(ident.map(|i: &str| Expression::Variable(i.into())))
            .padded();

        let op = |c| just(c).padded();

        let unary = op('-')
            .repeated()
            .foldr(atom, |_op, rhs| Expression::Neg(Box::new(rhs)));

        let power = unary.clone().foldl(
            op('^')
                .to(Expression::Power as fn(_, _) -> _)
                .then(unary)
                .repeated(),
            |lhs, (op, rhs)| op(Box::new(lhs), Box::new(rhs)),
        );

        let product = power.clone().foldl(
            choice((
                op('*').to(Expression::Product as fn(_, _) -> _),
                op('/').to(Expression::Quotient as fn(_, _) -> _),
            ))
            .then(power)
            .repeated(),
            |lhs, (op, rhs)| op(Box::new(lhs), Box::new(rhs)),
        );

        let sum = product.clone().foldl(
            choice((
                op('+').to(Expression::Sum as fn(_, _) -> _),
                op('-').to(Expression::Difference as fn(_, _) -> _),
            ))
            .then(product)
            .repeated(),
            |lhs, (op, rhs)| op(Box::new(lhs), Box::new(rhs)),
        );

        let eq = sum.clone().foldl(
            op('=')
                .to(Expression::Equal as fn(_, _) -> _)
                .then(sum)
                .repeated(),
            |lhs, (op, rhs)| op(Box::new(lhs), Box::new(rhs)),
        );

        eq
    });

    expr
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn parse_basics() {
        assert_eq!(
            Expression::parse("-6", true),
            Ok(Expression::Integer((-6).into()))
        );
        assert_eq!(
            Expression::parse("-a", true),
            Ok(Expression::Neg(Box::new(Expression::Variable("a".into()))))
        );
        assert_eq!(
            Expression::parse("a = 1", true),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("a".into())),
                Box::new(Expression::Integer(1.into())),
            ))
        );
        assert_eq!(
            Expression::parse("a^2", true),
            Ok(Expression::Power(
                Box::new(Expression::Variable("a".into())),
                Box::new(Expression::Integer(2.into())),
            ))
        );
        assert_eq!(
            Expression::parse("0.2", true),
            Ok(Expression::Rational(
                Rational::new(1.into(), 5.into()),
                false
            )),
        );

        assert_eq!(
            Expression::parse("x", true),
            Ok(Expression::Variable("x".into()))
        );
        assert_eq!(
            Expression::parse("2x", true),
            Ok(Expression::Product(
                Box::new(Expression::Integer(2.into())),
                Box::new(Expression::Variable("x".into())),
            ))
        );

        assert_eq!(
            Expression::parse("sqrt(2)", false),
            Ok(Expression::Sqrt(
                Box::new(Expression::Integer(2.into())),
                false
            ))
        );
        assert_eq!(
            Expression::parse("sqrt(2x)", true),
            Ok(Expression::Sqrt(
                Box::new(Expression::Product(
                    Box::new(Expression::Integer(2.into())),
                    Box::new(Expression::Variable("x".into())),
                )),
                false
            ))
        );
        assert_eq!(
            Expression::parse("abs(2)", false),
            Ok(Expression::Abs(Box::new(Expression::Integer(2.into()))))
        );
    }

    #[test]
    fn parse_complex() {
        // distance formula
        assert_eq!(
            Expression::parse("d = sqrt( (x2 - x1)^2 + (y2 - y1)^2 )", true),
            Ok(Expression::Equal(
                Box::new(Expression::Variable("d".into())),
                Box::new(Expression::Sqrt(
                    Box::new(Expression::Sum(
                        Box::new(Expression::Power(
                            Box::new(Expression::Difference(
                                Box::new(Expression::Variable("x2".into())),
                                Box::new(Expression::Variable("x1".into())),
                            )),
                            Box::new(Expression::Integer(2.into())),
                        ),),
                        Box::new(Expression::Power(
                            Box::new(Expression::Difference(
                                Box::new(Expression::Variable("y2".into())),
                                Box::new(Expression::Variable("y1".into())),
                            )),
                            Box::new(Expression::Integer(2.into())),
                        ),),
                    )),
                    false
                )),
            ))
        );

        // circle formula
        assert_eq!(
            Expression::parse("r^2 = (x-h)^2 + (y-k)^2", true),
            Ok(Expression::Equal(
                Box::new(Expression::Power(
                    Box::new(Expression::Variable("r".into())),
                    Box::new(Expression::Integer(2.into())),
                )),
                Box::new(Expression::Sum(
                    Box::new(Expression::Power(
                        Box::new(Expression::Difference(
                            Box::new(Expression::Variable("x".into())),
                            Box::new(Expression::Variable("h".into())),
                        )),
                        Box::new(Expression::Integer(2.into())),
                    )),
                    Box::new(Expression::Power(
                        Box::new(Expression::Difference(
                            Box::new(Expression::Variable("y".into())),
                            Box::new(Expression::Variable("k".into())),
                        )),
                        Box::new(Expression::Integer(2.into())),
                    ))
                ))
            ))
        );
    }
}
