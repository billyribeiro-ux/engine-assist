//! `when` / `fm:if` expressions (DR-027): `! & | ( )` over the twelve condition IDs, precedence
//! `!` > `&` > `|`, no spaces, nesting depth ≤ 64. Evaluated with Kleene logic in `rules`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::ConditionId;

/// Maximum nesting depth (SR-31, DR-015).
pub const MAX_DEPTH: usize = 64;

/// A parsed expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op", content = "args")]
pub enum Expr {
    /// A condition.
    Id(ConditionId),
    /// `!e`
    Not(Box<Expr>),
    /// `a & b`
    And(Box<Expr>, Box<Expr>),
    /// `a | b`
    Or(Box<Expr>, Box<Expr>),
}

/// An expression error with its 1-based column within the expression text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExprError {
    /// An identifier that isn't one of the twelve conditions (`grammar.unknown-condition`).
    #[error("unknown condition `{name}`")]
    UnknownCondition {
        /// The identifier.
        name: String,
        /// Column.
        column: usize,
    },
    /// Anything else (`grammar.bad-expression`).
    #[error("bad expression at column {column}: {message}")]
    Syntax {
        /// What went wrong.
        message: String,
        /// Column.
        column: usize,
    },
}

impl ExprError {
    /// The rule code.
    #[must_use]
    pub fn rule(&self) -> &'static str {
        match self {
            Self::UnknownCondition { .. } => "grammar.unknown-condition",
            Self::Syntax { .. } => "grammar.bad-expression",
        }
    }

    /// The 1-based column within the expression.
    #[must_use]
    pub fn column(&self) -> usize {
        match self {
            Self::UnknownCondition { column, .. } | Self::Syntax { column, .. } => *column,
        }
    }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

impl P<'_> {
    fn err(&self, m: &str) -> ExprError {
        ExprError::Syntax {
            message: m.to_owned(),
            column: self.i + 1,
        }
    }

    fn enter(&mut self) -> Result<(), ExprError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nested deeper than 64"));
        }
        Ok(())
    }

    fn or(&mut self) -> Result<Expr, ExprError> {
        self.enter()?;
        let mut e = self.and()?;
        while self.s.get(self.i) == Some(&b'|') {
            self.i += 1;
            let r = self.and()?;
            e = Expr::Or(Box::new(e), Box::new(r));
        }
        self.depth -= 1;
        Ok(e)
    }

    fn and(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.not()?;
        while self.s.get(self.i) == Some(&b'&') {
            self.i += 1;
            let r = self.not()?;
            e = Expr::And(Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn not(&mut self) -> Result<Expr, ExprError> {
        let mut n = 0usize;
        while self.s.get(self.i) == Some(&b'!') {
            self.i += 1;
            n += 1;
            if n > MAX_DEPTH {
                return Err(self.err("nested deeper than 64"));
            }
        }
        let mut e = self.atom()?;
        for _ in 0..n {
            e = Expr::Not(Box::new(e));
        }
        Ok(e)
    }

    fn atom(&mut self) -> Result<Expr, ExprError> {
        match self.s.get(self.i) {
            Some(b'(') => {
                self.i += 1;
                let e = self.or()?;
                if self.s.get(self.i) != Some(&b')') {
                    return Err(self.err("expected `)`"));
                }
                self.i += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_alphabetic() || *c == b'_' => {
                let start = self.i;
                while self
                    .s
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
                {
                    self.i += 1;
                }
                let name = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                ConditionId::parse(&name)
                    .map(Expr::Id)
                    .ok_or(ExprError::UnknownCondition {
                        name,
                        column: start + 1,
                    })
            }
            Some(_) => Err(self.err("expected a condition, `!` or `(`")),
            None => Err(self.err("unexpected end")),
        }
    }
}

/// Parses an expression.
///
/// # Errors
/// See [`ExprError`].
pub fn parse_expr(s: &str) -> Result<Expr, ExprError> {
    let mut p = P {
        s: s.as_bytes(),
        i: 0,
        depth: 0,
    };
    if s.is_empty() {
        return Err(p.err("empty expression"));
    }
    let e = p.or()?;
    if p.i != s.len() {
        return Err(p.err("unexpected character"));
    }
    Ok(e)
}

impl Expr {
    /// Every condition the expression names, in order of appearance.
    #[must_use]
    pub fn conditions(&self) -> Vec<ConditionId> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(e) = stack.pop() {
            match e {
                Self::Id(c) => out.push(*c),
                Self::Not(a) => stack.push(a),
                Self::And(a, b) | Self::Or(a, b) => {
                    stack.push(b);
                    stack.push(a);
                }
            }
        }
        out
    }

    fn prec(&self) -> u8 {
        match self {
            Self::Or(..) => 1,
            Self::And(..) => 2,
            Self::Not(_) | Self::Id(_) => 3,
        }
    }

    fn write(&self, f: &mut fmt::Formatter<'_>, min: u8) -> fmt::Result {
        let paren = self.prec() < min;
        if paren {
            f.write_str("(")?;
        }
        match self {
            Self::Id(c) => f.write_str(c.as_str())?,
            Self::Not(a) => {
                f.write_str("!")?;
                a.write(f, 3)?;
            }
            Self::And(a, b) => {
                a.write(f, 2)?;
                f.write_str("&")?;
                b.write(f, 3)?;
            }
            Self::Or(a, b) => {
                a.write(f, 1)?;
                f.write_str("|")?;
                b.write(f, 2)?;
            }
        }
        if paren {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(f, 0)
    }
}
