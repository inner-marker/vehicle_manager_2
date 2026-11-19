//! A formula's text read into an [`Expr`], and printed back.

use super::FormulaError;
use super::units::{SimpleUnit, Unit};

/// The longest formula, in characters.
pub const FORMULA_MAX_CHARS: usize = 500;

/// The most decimal places `round` takes.
pub const ROUND_MAX_PLACES: u8 = 6;

/// An arithmetic operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    /// `+`
    Plus,
    /// `−` (or `-`)
    Minus,
    /// `×` (or `*`)
    Times,
    /// `÷` (or `/`)
    Divide,
}

impl Op {
    const fn symbol(self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Minus => "−",
            Self::Times => "×",
            Self::Divide => "÷",
        }
    }

    const fn precedence(self) -> u8 {
        match self {
            Self::Plus | Self::Minus => 1,
            Self::Times | Self::Divide => 2,
        }
    }
}

/// A statistics function, over values listed in the formula or over past events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Function {
    /// The arithmetic mean.
    Mean,
    /// The middle value (the mean of the middle two, for an even count).
    Median,
    /// The geometric mean (every value above zero).
    GeometricMean,
    /// The harmonic mean (every value above zero): the right average for rates.
    HarmonicMean,
    /// The smallest.
    Min,
    /// The largest.
    Max,
    /// The total.
    Sum,
    /// How many (a plain number).
    Count,
    /// The sample standard deviation (at least two values).
    Stddev,
}

impl Function {
    /// Every function, in the menu's order.
    pub const ALL: [Self; 9] = [
        Self::Mean,
        Self::Median,
        Self::GeometricMean,
        Self::HarmonicMean,
        Self::Min,
        Self::Max,
        Self::Sum,
        Self::Count,
        Self::Stddev,
    ];

    /// How it's written.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mean => "mean",
            Self::Median => "median",
            Self::GeometricMean => "geometric_mean",
            Self::HarmonicMean => "harmonic_mean",
            Self::Min => "min",
            Self::Max => "max",
            Self::Sum => "sum",
            Self::Count => "count",
            Self::Stddev => "stddev",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|function| function.name() == name)
    }
}

/// Which past events a function works over: up to and including the event itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Over {
    /// All of them (`cumulative`).
    Cumulative,
    /// The last N (`last 5`, `last 5 events`).
    Events(u32),
    /// Those in the last N days (`last 30 days`).
    Days(u32),
    /// Those in the last N weeks.
    Weeks(u32),
    /// Those in the last N calendar months, in the household's time zone.
    Months(u32),
    /// Those within the last `amount` of `unit` (miles or kilometers) on `meter`, or on the
    /// vehicle's main odometer (`last 1000 mi`, `last 500 km on odometer`).
    Distance {
        /// How far back.
        amount: f64,
        /// Miles or kilometers.
        unit: SimpleUnit,
        /// The meter, if not the main odometer.
        meter: Option<String>,
    },
    /// Those within the last `amount` hours on `meter` (`last 50 h on engine-hours`).
    Hours {
        /// How many hours back.
        amount: f64,
        /// The hour meter (required when checked).
        meter: Option<String>,
    },
}

impl core::fmt::Display for Over {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let on = |meter: &Option<String>| {
            meter
                .as_ref()
                .map_or_else(String::new, |meter| format!(" on {meter}"))
        };
        match self {
            Self::Cumulative => f.write_str("cumulative"),
            Self::Events(n) => write!(f, "last {n}"),
            Self::Days(n) => write!(f, "last {n} days"),
            Self::Weeks(n) => write!(f, "last {n} weeks"),
            Self::Months(n) => write!(f, "last {n} months"),
            Self::Distance {
                amount,
                unit,
                meter,
            } => write!(f, "last {amount} {}{}", unit.word(), on(meter)),
            Self::Hours { amount, meter } => write!(f, "last {amount} h{}", on(meter)),
        }
    }
}

/// A formula, read.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A number, as typed, with its unit if any (`33 mi/h`).
    Number {
        /// The number as typed (in `unit`).
        value: f64,
        /// Its unit.
        unit: Option<Unit>,
    },
    /// A value by its key (`cost`, `trip-time`, `odometer`).
    Key(String),
    /// How far a meter moved since the last event of this kind
    /// (`distance-since-last(odometer)`).
    SinceLast(String),
    /// How long since the last event of this kind (`time-since-last`).
    TimeSinceLast,
    /// `−x`
    Negate(Box<Expr>),
    /// `x + y`, `x × y`, …
    Binary {
        /// The operation.
        op: Op,
        /// The left side.
        left: Box<Expr>,
        /// The right side.
        right: Box<Expr>,
    },
    /// A function over listed values: `mean(a, b, c)`.
    Call {
        /// Which.
        function: Function,
        /// The values (at least one).
        args: Vec<Expr>,
    },
    /// A function over past events: `mean(trip-time, last 5)`.
    Over {
        /// Which.
        function: Function,
        /// Worked out on each event.
        expr: Box<Expr>,
        /// Which events.
        over: Over,
    },
    /// `round(x, places)`: rounded as shown, in the person's units.
    Round {
        /// The value.
        expr: Box<Expr>,
        /// Decimal places, 0–6.
        places: u8,
    },
    /// `abs(x)`
    Abs(Box<Expr>),
}

impl Expr {
    const fn precedence(&self) -> u8 {
        match self {
            Self::Binary { op, .. } => op.precedence(),
            Self::Negate(_) => 3,
            Self::Number { .. }
            | Self::Key(_)
            | Self::SinceLast(_)
            | Self::TimeSinceLast
            | Self::Call { .. }
            | Self::Over { .. }
            | Self::Round { .. }
            | Self::Abs(_) => 4,
        }
    }

    /// Calls `visit` on this and every expression inside it.
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Self)) {
        visit(self);
        match self {
            Self::Number { .. } | Self::Key(_) | Self::SinceLast(_) | Self::TimeSinceLast => {}
            Self::Negate(inner)
            | Self::Abs(inner)
            | Self::Round { expr: inner, .. }
            | Self::Over { expr: inner, .. } => inner.walk(visit),
            Self::Binary { left, right, .. } => {
                left.walk(visit);
                right.walk(visit);
            }
            Self::Call { args, .. } => args.iter().for_each(|arg| arg.walk(visit)),
        }
    }
}

impl core::fmt::Display for Expr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Number { value, unit } => match unit {
                Some(Unit::Percent) => write!(f, "{value}%"),
                Some(unit) => write!(f, "{value} {unit}"),
                None => write!(f, "{value}"),
            },
            Self::Key(key) => f.write_str(key),
            Self::SinceLast(key) => write!(f, "distance-since-last({key})"),
            Self::TimeSinceLast => f.write_str("time-since-last"),
            Self::Negate(inner) => {
                if inner.precedence() < 3 {
                    write!(f, "−({inner})")
                } else {
                    write!(f, "−{inner}")
                }
            }
            Self::Binary { op, left, right } => {
                let side = |f: &mut core::fmt::Formatter<'_>, side: &Self, tighter: bool| {
                    let needs = side.precedence() < op.precedence()
                        || (tighter && side.precedence() == op.precedence());
                    if needs {
                        write!(f, "({side})")
                    } else {
                        write!(f, "{side}")
                    }
                };
                side(f, left, false)?;
                write!(f, " {} ", op.symbol())?;
                side(f, right, true)
            }
            Self::Call { function, args } => {
                let args: Vec<String> = args.iter().map(ToString::to_string).collect();
                write!(f, "{}({})", function.name(), args.join(", "))
            }
            Self::Over {
                function,
                expr,
                over,
            } => write!(f, "{}({expr}, {over})", function.name()),
            Self::Round { expr, places } => write!(f, "round({expr}, {places})"),
            Self::Abs(inner) => write!(f, "abs({inner})"),
        }
    }
}

// --- Tokens ------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Word(String),
    Op(Op),
    Open,
    Close,
    Comma,
    Percent,
    /// `/`: a division, or the `/` in a unit (`mi/h`); `÷` is only ever a division.
    Slash,
}

impl Token {
    fn text(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Word(word) => word.clone(),
            Self::Op(op) => op.symbol().to_owned(),
            Self::Open => "(".to_owned(),
            Self::Close => ")".to_owned(),
            Self::Comma => ",".to_owned(),
            Self::Percent => "%".to_owned(),
            Self::Slash => "/".to_owned(),
        }
    }
}

/// `text` as tokens, each with the character it starts at.
fn tokens(text: &str) -> Result<Vec<(usize, Token)>, FormulaError> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        let start = at;
        at += 1;
        let token = match c {
            c if c.is_whitespace() => continue,
            '+' => Token::Op(Op::Plus),
            '-' | '−' => Token::Op(Op::Minus),
            '*' | '×' | '·' => Token::Op(Op::Times),
            '÷' => Token::Op(Op::Divide),
            '/' => Token::Slash,
            '(' => Token::Open,
            ')' => Token::Close,
            ',' => Token::Comma,
            '%' => Token::Percent,
            c if c.is_ascii_digit() || c == '.' => {
                while chars
                    .get(at)
                    .is_some_and(|c| c.is_ascii_digit() || *c == '.')
                {
                    at += 1;
                }
                let number: String = chars[start..at].iter().collect();
                let value = number
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or(FormulaError::Unexpected {
                        at: start,
                        found: number,
                    })?;
                Token::Number(value)
            }
            c if c.is_ascii_alphabetic() => {
                // A key or a word: letters, digits, `-` and `_` (a `-` only before a letter
                // or digit, so `a -b` and `a-` read as a minus).
                while let Some(&next) = chars.get(at) {
                    let joins = next.is_ascii_alphanumeric()
                        || next == '_'
                        || (next == '-'
                            && chars.get(at + 1).is_some_and(char::is_ascii_alphanumeric));
                    if !joins {
                        break;
                    }
                    at += 1;
                }
                Token::Word(chars[start..at].iter().collect())
            }
            other => {
                return Err(FormulaError::Unexpected {
                    at: start,
                    found: other.to_string(),
                });
            }
        };
        found.push((start, token));
    }
    Ok(found)
}

// --- Parsing -----------------------------------------------------------------------------

/// Reads `text` (already trimmed, not empty, not too long).
pub(super) fn parse(text: &str) -> Result<Expr, FormulaError> {
    let mut parser = Parser {
        tokens: tokens(text)?,
        next: 0,
        end: text.chars().count(),
    };
    let expr = parser.expr()?;
    match parser.peek() {
        None => Ok(expr),
        Some(_) => Err(parser.unexpected()),
    }
}

struct Parser {
    tokens: Vec<(usize, Token)>,
    next: usize,
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next).map(|(_, token)| token)
    }

    fn peek_word(&self) -> Option<&str> {
        match self.peek() {
            Some(Token::Word(word)) => Some(word),
            _ => None,
        }
    }

    fn at(&self) -> usize {
        self.tokens.get(self.next).map_or(self.end, |(at, _)| *at)
    }

    fn take(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.next).map(|(_, token)| token.clone());
        self.next += 1;
        token
    }

    fn unexpected(&self) -> FormulaError {
        match self.tokens.get(self.next) {
            Some((at, token)) => FormulaError::Unexpected {
                at: *at,
                found: token.text(),
            },
            None => FormulaError::UnexpectedEnd,
        }
    }

    fn expect(&mut self, wanted: &Token) -> Result<(), FormulaError> {
        if self.peek() == Some(wanted) {
            self.next += 1;
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    fn expr(&mut self) -> Result<Expr, FormulaError> {
        self.binary(1)
    }

    /// Operations of `level` (1: `+ −`, 2: `× ÷`) and tighter, left to right.
    fn binary(&mut self, level: u8) -> Result<Expr, FormulaError> {
        let mut left = if level == 1 {
            self.binary(2)?
        } else {
            self.unary()?
        };
        loop {
            let op = match self.peek() {
                Some(Token::Op(op)) => *op,
                Some(Token::Slash) => Op::Divide,
                _ => break,
            };
            if op.precedence() != level {
                break;
            }
            self.next += 1;
            let right = if level == 1 {
                self.binary(2)?
            } else {
                self.unary()?
            };
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, FormulaError> {
        if self.peek() == Some(&Token::Op(Op::Minus)) {
            self.next += 1;
            return Ok(Expr::Negate(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, FormulaError> {
        let at = self.at();
        match self.take() {
            Some(Token::Number(value)) => Ok(Expr::Number {
                value,
                unit: self.unit(),
            }),
            Some(Token::Open) => {
                let inner = self.expr()?;
                if self.peek() == Some(&Token::Close) {
                    self.next += 1;
                    Ok(inner)
                } else if self.peek().is_none() {
                    Err(FormulaError::Unclosed { at })
                } else {
                    Err(self.unexpected())
                }
            }
            Some(Token::Word(word)) => {
                if self.peek() == Some(&Token::Open) {
                    self.next += 1;
                    self.call(&word, at)
                } else if word == "time-since-last" {
                    Ok(Expr::TimeSinceLast)
                } else {
                    Ok(Expr::Key(word))
                }
            }
            _ => {
                self.next -= 1;
                Err(self.unexpected())
            }
        }
    }

    /// A number's unit, if one follows it: `%`, `mi`, `mi/h`.
    fn unit(&mut self) -> Option<Unit> {
        if self.peek() == Some(&Token::Percent) {
            self.next += 1;
            return Some(Unit::Percent);
        }
        let top = self.peek_word().and_then(SimpleUnit::from_word)?;
        self.next += 1;
        let per = self.peek() == Some(&Token::Slash);
        let bottom = self.tokens.get(self.next + 1).and_then(|(_, token)| {
            if let Token::Word(word) = token {
                SimpleUnit::from_word(word)
            } else {
                None
            }
        });
        match (per, bottom) {
            (true, Some(bottom)) => {
                self.next += 2;
                Some(Unit::Per(top, bottom))
            }
            _ => Some(Unit::Simple(top)),
        }
    }

    /// The rest of `name(…` (the `(` taken).
    fn call(&mut self, name: &str, at: usize) -> Result<Expr, FormulaError> {
        let expr = match name {
            "abs" => Expr::Abs(Box::new(self.expr()?)),
            "round" => {
                let inner = self.expr()?;
                self.expect(&Token::Comma)?;
                let places = match self.take() {
                    Some(Token::Number(places))
                        if places.fract().abs() < f64::EPSILON
                            && (0.0..=f64::from(ROUND_MAX_PLACES)).contains(&places) =>
                    {
                        // Checked whole and in range just above.
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "a whole number from 0 to 6"
                        )]
                        let places = places as u8;
                        places
                    }
                    _ => return Err(FormulaError::BadPlaces),
                };
                Expr::Round {
                    expr: Box::new(inner),
                    places,
                }
            }
            "distance-since-last" => {
                let Some(Token::Word(key)) = self.take() else {
                    self.next -= 1;
                    return Err(self.unexpected());
                };
                Expr::SinceLast(key)
            }
            "time-since-last" => Expr::TimeSinceLast,
            _ => {
                let function = Function::from_name(name).ok_or(FormulaError::UnknownFunction {
                    at,
                    name: name.to_owned(),
                })?;
                return self.function(function);
            }
        };
        self.expect(&Token::Close)?;
        Ok(expr)
    }

    /// A statistics function's values, or its expression and Over, and the `)`.
    fn function(&mut self, function: Function) -> Result<Expr, FormulaError> {
        let mut args = vec![self.expr()?];
        while self.peek() == Some(&Token::Comma) {
            self.next += 1;
            if args.len() == 1 && matches!(self.peek_word(), Some("last" | "cumulative")) {
                let over = self.over()?;
                self.expect(&Token::Close)?;
                return Ok(Expr::Over {
                    function,
                    expr: Box::new(args.remove(0)),
                    over,
                });
            }
            args.push(self.expr()?);
        }
        self.expect(&Token::Close)?;
        Ok(Expr::Call { function, args })
    }

    /// `cumulative`, or `last N [events|days|weeks|months|mi|km|h] [on key]`.
    fn over(&mut self) -> Result<Over, FormulaError> {
        if self.peek_word() == Some("cumulative") {
            self.next += 1;
            return Ok(Over::Cumulative);
        }
        self.next += 1; // `last`
        let Some(Token::Number(amount)) = self.peek().cloned() else {
            return Err(self.unexpected());
        };
        if amount <= 0.0 {
            return Err(self.unexpected());
        }
        self.next += 1;
        let whole = || -> Option<u32> {
            (amount.fract().abs() < f64::EPSILON && amount <= f64::from(u32::MAX)).then(|| {
                // Checked whole, above zero, and in range just above.
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a whole number in u32's range"
                )]
                let n = amount as u32;
                n
            })
        };
        let unit_at = self.at();
        let over = match self.peek_word() {
            None | Some("on") => Over::Events(whole().ok_or_else(|| self.unexpected())?),
            Some(word) => {
                let over = match word {
                    "event" | "events" => whole().map(Over::Events),
                    "day" | "days" => whole().map(Over::Days),
                    "week" | "weeks" => whole().map(Over::Weeks),
                    "month" | "months" => whole().map(Over::Months),
                    word => match SimpleUnit::from_word(word) {
                        Some(unit @ (SimpleUnit::Miles | SimpleUnit::Kilometers)) => {
                            Some(Over::Distance {
                                amount,
                                unit,
                                meter: None,
                            })
                        }
                        Some(SimpleUnit::Hours) => Some(Over::Hours {
                            amount,
                            meter: None,
                        }),
                        _ => None,
                    },
                };
                let over = over.ok_or_else(|| FormulaError::Unexpected {
                    at: unit_at,
                    found: word.to_owned(),
                })?;
                self.next += 1;
                over
            }
        };
        self.on_meter(over, unit_at)
    }

    /// `over`, with the meter named by `on key` if that follows (`unit_at`: where its unit
    /// was, for a span that can't take a meter).
    fn on_meter(&mut self, over: Over, unit_at: usize) -> Result<Over, FormulaError> {
        if self.peek_word() != Some("on") {
            return Ok(over);
        }
        self.next += 1;
        let Some(Token::Word(key)) = self.take() else {
            self.next -= 1;
            return Err(self.unexpected());
        };
        match over {
            Over::Distance { amount, unit, .. } => Ok(Over::Distance {
                amount,
                unit,
                meter: Some(key),
            }),
            Over::Hours { amount, .. } => Ok(Over::Hours {
                amount,
                meter: Some(key),
            }),
            Over::Cumulative
            | Over::Events(_)
            | Over::Days(_)
            | Over::Weeks(_)
            | Over::Months(_) => Err(FormulaError::Unexpected {
                at: unit_at,
                found: "on".to_owned(),
            }),
        }
    }
}
