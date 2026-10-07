pub mod disjoint;

use itertools::Itertools;
use jade_core::piece::Piece;
use jade_core::queue::CAP;
use jade_core::queue::Queue;
use std::collections::BTreeSet;
use std::fmt::Display;

// X;Y
#[derive(Clone, PartialEq, Debug)]
pub struct Pattern(pub Vec<Segment>);



#[derive(Clone, PartialEq, Debug)]
pub enum Segment {
    Single(Piece),                // X
    Sequence(Vec<Self>),          // XY or X,Y
    Group(Box<Self>),             // (X)
    Bag(Vec<Self>),               // [XYZ], the set of pieces X, Y, and Z
    Except(Vec<Self>),            // [^XYZ], which is [TIJLOSZ] \ {X, Y, Z}
    Wildcard,                     // *, is exactly [TIJLOSZ]
    Permute(Box<Self>, usize),    // XpN, returns all permutations of N elements from X
    Choose(Box<Self>, usize),     // XcN, returns all combinations of N elements from X
    All(Box<Self>),               // X! returns all permutations of all elements in X
    Filter(Box<Self>, Condition), // X{C} returns all elements of X that satisfy condition C
}

#[derive(Clone, PartialEq, Debug)]
pub enum Condition {
    And(Vec<Self>),                                // X & Y
    Or(Vec<Self>),                                 // X | Y
    Not(Box<Self>),                                // !X
    Order(Box<Pattern>, Box<Pattern>, Comparator), // X < Y, X > Y, X <= Y, X >= Y, X == Y, X != Y
    Count(Box<Pattern>, usize, Comparator),        // X < N, X > N, X <= N, X >= N, X == N, X != N
    Exists(Box<Pattern>),                          // X exists, i.e. count(X) > 0
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Comparator {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

#[derive(Debug)]
pub enum PatternError {
    ParseError(String),
    InvalidPattern(String),
}

impl std::error::Error for PatternError {}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternError::ParseError(msg) => write!(f, "Parse error: {msg}"),
            PatternError::InvalidPattern(msg) => write!(f, "Invalid pattern: {msg}"),
        }
    }
}

impl Pattern {
    #[must_use]
    pub fn expand(&self) -> BTreeSet<Queue> {
        let mut result = BTreeSet::new();

        for segment in &self.0 {
            result.extend(segment.expand());
        }

        result
    }

    /// Parse a pattern from its textual form (see the grammar docs).
    pub fn parse(src: &str) -> Result<Self, PatternError> {
        let mut parser = Parser::new(src);
        let pattern = parser.parse_pattern()?;
        parser.skip_ws();
        if parser.pos != parser.chars.len() {
            return Err(PatternError::ParseError(format!(
                "unexpected '{}' at character {}",
                parser.chars[parser.pos], parser.pos
            )));
        }
        Ok(pattern)
    }

    /// The top-level `X;Y` segments that make up the pattern.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.0
    }

    /// The AST size of this node.
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        let mut size = 1;
        for segment in &self.0 {
            size += segment.size();
        }
        size
    }
}

impl std::str::FromStr for Pattern {
    type Err = PatternError;

    fn from_str(src: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(src)
    }
}

impl Segment {
    #[must_use]
    pub fn expand(&self) -> Vec<Queue> {
        match self {
            Self::Single(piece) => vec![Queue::from_slice(&[*piece])],

            // AB is a product of A and B, [TI][JL] should produce TJ;TL;IJ;IL
            Self::Sequence(segments) => {
                let mut result = vec![Queue::new()];
                for segment in segments {
                    let expanded = segment.expand();
                    let mut new_result = Vec::new();
                    for prefix in &result {
                        for suffix in &expanded {
                            let mut combined = *prefix;
                            combined.extend(*suffix);
                            new_result.push(combined);
                        }
                    }
                    result = new_result;
                }
                result
            }

            Self::Group(inner) => inner.expand(),
            Self::Bag(inner) => {
                let mut result = Vec::new();
                for segment in inner {
                    result.extend(segment.expand());
                }
                result
            }

            Self::Wildcard => Piece::ALL.iter().copied().map(one).collect(),

            Self::Except(inner) => {
                let mut excluded = Vec::new();
                for segment in inner {
                    for exp in segment.expand() {
                        excluded.extend(exp);
                    }
                }
                let mut result = Vec::new();
                for piece in Piece::ALL {
                    if !excluded.contains(&piece) {
                        result.push(one(piece));
                    }
                }
                result
            }

            Self::Permute(expr, n) => expr
                .expand()
                .into_iter()
                .permutations(*n)
                .map(|perm| Queue::from_iter(perm.into_iter().flatten()))
                .unique()
                .collect(),

            Self::Choose(expr, n) => expr
                .expand()
                .into_iter()
                .combinations(*n)
                .map(|comb| Queue::from_iter(comb.into_iter().flatten()))
                .unique()
                .collect(),

            Self::All(expr) => {
                let mut elements = Vec::new();
                for exp in expr.expand() {
                    elements.extend(exp);
                }
                let len = elements.len();
                elements
                    .into_iter()
                    .permutations(len)
                    .map(Queue::from_iter)
                    .unique()
                    .collect()
            }

            Self::Filter(a, c) => a
                .expand()
                .into_iter()
                .filter(|exp| c.evaluate(exp))
                .collect(),
        }
    }

    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        match self {
            Segment::All(z) | Segment::Choose(z, _) => 1 + z.size(),
            Segment::Bag(z) | Segment::Except(z) => 1 + z.iter().map(Segment::size).sum::<usize>(),
            Segment::Filter(a, c) => 1 + a.size() + c.size(),
            Segment::Group(a) | Segment::Permute(a, _) => 1 + a.size(),
            Segment::Sequence(a) => 1 + a.iter().map(Segment::size).sum::<usize>(),
            Segment::Single(_) | Segment::Wildcard => 1,
        }
    }
}

impl Condition {
    #[must_use]
    pub fn evaluate(&self, pieces: &[Piece]) -> bool {
        match self {
            Self::And(conditions) => conditions.iter().all(|c| c.evaluate(pieces)),
            Self::Or(conditions) => conditions.iter().any(|c| c.evaluate(pieces)),
            Self::Not(condition) => !condition.evaluate(pieces),
            Self::Order(a, b, comparator) => {
                let aw = occurrences(a, pieces);
                let bw = occurrences(b, pieces);
                if aw.is_empty() || bw.is_empty() {
                    return false;
                }
                match comparator {
                    Comparator::Eq | Comparator::Ne => {
                        let same = aw.iter().any(|wa| bw.iter().any(|wb| wa == wb));
                        match comparator {
                            Comparator::Eq => same,
                            _ => !same,
                        }
                    }
                    Comparator::Lt | Comparator::Le => {
                        aw.iter().any(|wa| bw.iter().any(|wb| wa.1 <= wb.0))
                    }
                    Comparator::Gt | Comparator::Ge => {
                        bw.iter().any(|wb| aw.iter().any(|wa| wb.1 <= wa.0))
                    }
                }
            }
            Self::Count(a, n, comparator) => {
                let count = count_occurrences(a, pieces);
                compare_count(count, *n, *comparator)
            }
            Self::Exists(a) => count_occurrences(a, pieces) > 0,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Condition::And(conditions) | Condition::Or(conditions) => {
                1 + conditions.iter().map(Condition::size).sum::<usize>()
            }
            Condition::Not(condition) => 1 + condition.size(),
            Condition::Order(a, b, _) => 1 + a.size() + b.size(),
            Condition::Count(a, _, _) | Condition::Exists(a) => 1 + a.size(),
        }
    }
}

/// The end index of the longest expansion of `pattern` matching
/// `pieces[start..]`, if any.
fn matches_at(pattern: &Pattern, pieces: &[Piece], start: usize) -> Option<usize> {
    let mut longest = None;
    for expansion in pattern.expand() {
        let len = expansion.len();
        if len == 0 || start + len > pieces.len() {
            continue;
        }
        if &pieces[start..start + len] == expansion.as_slice()
            && longest.is_none_or(|best: usize| len > best - start)
        {
            longest = Some(start + len);
        }
    }
    longest
}

/// Every (start, end) window of `pieces` matched by some expansion of
/// `pattern`.
fn occurrences(pattern: &Pattern, pieces: &[Piece]) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    for expansion in pattern.expand() {
        let len = expansion.len();
        if len == 0 || len > pieces.len() {
            continue;
        }
        for start in 0..=(pieces.len() - len) {
            if &pieces[start..start + len] == expansion.as_slice() {
                result.push((start, start + len));
            }
        }
    }
    result.sort_unstable();
    result.dedup();
    result
}

/// The number of non-overlapping matches of `pattern` in `pieces`, counting
/// greedily left-to-right with the longest match taken at each position.
fn count_occurrences(pattern: &Pattern, pieces: &[Piece]) -> usize {
    let mut count = 0;
    let mut pos = 0;
    while pos < pieces.len() {
        match matches_at(pattern, pieces, pos) {
            Some(end) => {
                count += 1;
                pos = end;
            }
            None => pos += 1,
        }
    }
    count
}

#[must_use]
pub const fn compare_count(count: usize, n: usize, comparator: Comparator) -> bool {
    match comparator {
        Comparator::Eq => count == n,
        Comparator::Ne => count != n,
        Comparator::Lt => count < n,
        Comparator::Le => count <= n,
        Comparator::Gt => count > n,
        Comparator::Ge => count >= n,
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

/// Returns the queue holding the single piece `piece`.
const fn one(piece: Piece) -> Queue {
    Queue::from_slice(&[piece])
}

/// The length of the longest expansion of `segment`.
///
/// A queue holds at most [`CAP`] pieces, so a segment with a longer expansion
/// cannot be built. The sum for a sequence saturates, so a long source cannot
/// overflow.
fn max_len(segment: &Segment) -> usize {
    match segment {
        // Each item of an `Except` excludes pieces, so every expansion of it
        // holds one piece.
        Segment::Single(_) | Segment::Wildcard | Segment::Except(_) => 1,
        Segment::Group(inner) | Segment::Filter(inner, _) | Segment::All(inner) => max_len(inner),
        Segment::Sequence(segments) => segments
            .iter()
            .map(max_len)
            .fold(0, usize::saturating_add),
        Segment::Bag(items) => items.iter().map(max_len).max().unwrap_or(0),
        Segment::Permute(inner, n) => max_len(inner).min(*n),
        Segment::Choose(inner, n) => largest_sum(inner, *n),
    }
}

/// The sum of the `n` longest expansions of `segment`, or of every expansion
/// when `segment` has fewer than `n`.
///
/// `Choose` concatenates `n` expansions, so its longest result takes the `n`
/// longest ones. Their lengths are only known by expanding. That expansion is
/// no larger than the one the caller asks for later.
fn largest_sum(segment: &Segment, n: usize) -> usize {
    let mut lengths: Vec<usize> = segment.expand().iter().map(Queue::len).collect();
    lengths.sort_unstable_by(|a, b| b.cmp(a));
    lengths.iter().take(n).sum()
}

fn seq_segment(terms: Vec<Segment>) -> Segment {
    match terms.len() {
        1 => terms.into_iter().next().unwrap(),
        _ => Segment::Sequence(terms),
    }
}

impl Parser {
    fn new(src: &str) -> Self {
        Self {
            chars: src.chars().collect(),
            pos: 0,
        }
    }

    fn error<T>(&self, msg: impl std::fmt::Display) -> Result<T, PatternError> {
        Err(PatternError::ParseError(format!(
            "{msg} at character {}",
            self.pos
        )))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) -> Result<(), PatternError> {
        if self.eat(c) {
            Ok(())
        } else {
            self.error(format!("expected '{c}'"))
        }
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|c| c.is_whitespace() && c != '\n') {
            self.pos += 1;
        }
    }

    fn digits(&mut self) -> Result<usize, PatternError> {
        let mut n: usize = 0;
        let mut seen = false;
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            seen = true;
            let d = c.to_digit(10).unwrap() as usize;
            n = n
                .checked_mul(10)
                .and_then(|v| v.checked_add(d))
                .ok_or_else(|| {
                    PatternError::ParseError(format!("number too large at character {}", self.pos))
                })?;
            self.pos += 1;
        }
        if seen {
            Ok(n)
        } else {
            self.error("expected digits")
        }
    }

    fn parse_pattern(&mut self) -> Result<Pattern, PatternError> {
        self.skip_ws();
        let mut segments = vec![self.parse_sequence()?];
        loop {
            self.skip_ws();
            if self.eat(';') {
                self.skip_ws_after_newline();
                segments.push(self.parse_sequence()?);
                continue;
            }
            if self.eat('\n') {
                self.skip_ws_after_newline();
                if self.peek().is_none() {
                    break;
                }
                segments.push(self.parse_sequence()?);
                continue;
            }
            break;
        }

        // The top-level segments are alternatives, so the longest queue is the
        // longest of them, not their sum.
        let longest = segments.iter().map(max_len).max().unwrap_or(0);
        if longest > CAP {
            return Err(PatternError::InvalidPattern(format!(
                "pattern expands to a queue of {longest} pieces, a queue holds {CAP}"
            )));
        }
        Ok(Pattern(segments))
    }

    /// Skip whitespace including newlines, used after a newline separator so
    /// leading blank lines and runs of newlines don't create empty segments.
    fn skip_ws_after_newline(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn parse_sequence(&mut self) -> Result<Segment, PatternError> {
        self.skip_ws();
        let mut terms = vec![self.parse_concatenation()?];
        loop {
            self.skip_ws();
            if !self.eat(',') {
                break;
            }
            terms.push(self.parse_concatenation()?);
        }
        Ok(seq_segment(terms))
    }

    /// A run of juxtaposed terms. Stops at any token that cannot continue a
    /// term.
    fn parse_concatenation(&mut self) -> Result<Segment, PatternError> {
        self.skip_ws();
        let mut terms = vec![self.parse_term()?];
        while let Some(c) = self.peek() {
            match c {
                ';' | '\n' | ',' | ']' | '}' | ')' | '&' | '|' | '<' | '>' | '=' | '!' => break,
                _ => {}
            }
            terms.push(self.parse_term()?);
        }
        Ok(seq_segment(terms))
    }

    fn parse_term(&mut self) -> Result<Segment, PatternError> {
        self.skip_ws();
        let mut segment = self.parse_atom()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('p') => {
                    self.pos += 1;
                    let n = self.digits()?;
                    segment = Segment::Permute(Box::new(segment), n);
                }
                Some('c') => {
                    self.pos += 1;
                    let n = self.digits()?;
                    segment = Segment::Choose(Box::new(segment), n);
                }
                // '!' followed by '=' starts a comparison inside a condition, so it
                // cannot be the All postfix.
                Some('!') if self.peek_next() != Some('=') => {
                    self.pos += 1;
                    segment = Segment::All(Box::new(segment));
                }
                Some('{') => {
                    self.pos += 1;
                    let condition = self.parse_condition()?;
                    self.expect('}')?;
                    segment = Segment::Filter(Box::new(segment), condition);
                }
                _ => break,
            }
        }
        Ok(segment)
    }

    fn parse_atom(&mut self) -> Result<Segment, PatternError> {
        self.skip_ws();
        match self.peek() {
            Some(c) if c.is_alphabetic() => {
                let piece = match c.to_ascii_uppercase() {
                    'T' => Piece::T,
                    'I' => Piece::I,
                    'J' => Piece::J,
                    'L' => Piece::L,
                    'O' => Piece::O,
                    'S' => Piece::S,
                    'Z' => Piece::Z,
                    _ => return self.error(format!("unknown piece '{c}'")),
                };
                self.pos += 1;
                Ok(Segment::Single(piece))
            }
            Some('*') => {
                self.pos += 1;
                Ok(Segment::Wildcard)
            }
            Some('(') => {
                self.pos += 1;
                let inner = self.parse_sequence()?;
                self.expect(')')?;
                Ok(Segment::Group(Box::new(inner)))
            }
            Some('[') => self.parse_bag(),
            Some(c) => self.error(format!("expected an atom, found '{c}'")),
            None => self.error("unexpected end of input, expected an atom"),
        }
    }

    fn parse_bag(&mut self) -> Result<Segment, PatternError> {
        self.expect('[')?;
        let except = self.eat('^');
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(']') {
                break;
            }
            items.push(self.parse_term()?);
            self.skip_ws();
            if self.peek() == Some(']') {
                break;
            }
            self.eat(',');
        }
        self.expect(']')?;
        if items.is_empty() {
            return self.error("empty bag");
        }
        Ok(if except {
            Segment::Except(items)
        } else {
            Segment::Bag(items)
        })
    }

    fn parse_condition(&mut self) -> Result<Condition, PatternError> {
        self.skip_ws();
        let mut parts = vec![self.parse_and_term()?];
        loop {
            self.skip_ws();
            if !self.eat('|') {
                break;
            }
            parts.push(self.parse_and_term()?);
        }
        Ok(match parts.len() {
            1 => parts.pop().unwrap(),
            _ => Condition::Or(parts),
        })
    }

    fn parse_and_term(&mut self) -> Result<Condition, PatternError> {
        self.skip_ws();
        let mut parts = vec![self.parse_unary()?];
        loop {
            self.skip_ws();
            if !self.eat('&') {
                break;
            }
            parts.push(self.parse_unary()?);
        }
        Ok(match parts.len() {
            1 => parts.pop().unwrap(),
            _ => Condition::And(parts),
        })
    }

    fn parse_unary(&mut self) -> Result<Condition, PatternError> {
        self.skip_ws();
        if self.eat('!') {
            Ok(Condition::Not(Box::new(self.parse_unary()?)))
        } else if self.peek() == Some('(') {
            self.pos += 1;
            let inner = self.parse_condition()?;
            self.expect(')')?;
            Ok(inner)
        } else {
            self.parse_comparison()
        }
    }

    fn parse_comparison(&mut self) -> Result<Condition, PatternError> {
        self.skip_ws();
        let lhs = Box::new(self.parse_pattern()?);
        self.skip_ws();
        let comparator = self.parse_relop()?;
        self.skip_ws();
        if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            let n = self.digits()?;
            Ok(Condition::Count(lhs, n, comparator))
        } else {
            let rhs = self.parse_pattern()?;
            Ok(Condition::Order(lhs, Box::new(rhs), comparator))
        }
    }

    fn parse_relop(&mut self) -> Result<Comparator, PatternError> {
        self.skip_ws();
        let c = self.peek().ok_or_else(|| {
            PatternError::ParseError(format!(
                "expected a comparison, found end of input at character {}",
                self.pos
            ))
        })?;
        let (comparator, len) = match (c, self.peek_next()) {
            ('=', Some('=')) => (Comparator::Eq, 2),
            ('!', Some('=')) => (Comparator::Ne, 2),
            ('<', Some('=')) => (Comparator::Le, 2),
            ('>', Some('=')) => (Comparator::Ge, 2),
            ('<', _) => (Comparator::Lt, 1),
            ('>', _) => (Comparator::Gt, 1),
            _ => return self.error(format!("expected a comparison operator, found '{c}'")),
        };
        self.pos += len;
        Ok(comparator)
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let segments: Vec<String> = self
            .0
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        write!(f, "{}", segments.join(";"))
    }
}

impl Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Single(piece) => write!(f, "{piece}"),
            Self::Sequence(segments) => {
                let segments: Vec<String> = segments
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                write!(f, "{}", segments.join(""))
            }
            Self::Group(inner) => write!(f, "({inner})"),
            Self::Bag(inner) => {
                let inner: Vec<String> =
                    inner.iter().map(std::string::ToString::to_string).collect();
                write!(f, "[{}]", inner.join(""))
            }
            Self::Except(inner) => {
                let inner: Vec<String> =
                    inner.iter().map(std::string::ToString::to_string).collect();
                write!(f, "[^{}]", inner.join(""))
            }
            Self::Wildcard => write!(f, "*"),
            Self::Permute(inner, n) => write!(f, "{inner}p{n}"),
            Self::Choose(inner, n) => write!(f, "{inner}c{n}"),
            Self::All(inner) => write!(f, "{inner}!"),
            Self::Filter(inner, condition) => write!(f, "{inner}{{{condition}}}"),
        }
    }
}

impl Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::And(conditions) => {
                let conditions: Vec<String> = conditions
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                write!(f, "{}", conditions.join("&"))
            }
            Self::Or(conditions) => {
                let conditions: Vec<String> = conditions
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                write!(f, "{}", conditions.join("|"))
            }
            Self::Not(condition) => write!(f, "!{condition}"),
            Self::Order(lhs, rhs, comparator) => {
                let op = match comparator {
                    Comparator::Eq => "==",
                    Comparator::Ne => "!=",
                    Comparator::Lt => "<",
                    Comparator::Le => "<=",
                    Comparator::Gt => ">",
                    Comparator::Ge => ">=",
                };
                write!(f, "{lhs}{op}{rhs}")
            }
            Self::Count(pattern, n, comparator) => {
                let op = match comparator {
                    Comparator::Eq => "==",
                    Comparator::Ne => "!=",
                    Comparator::Lt => "<",
                    Comparator::Le => "<=",
                    Comparator::Gt => ">",
                    Comparator::Ge => ">=",
                };
                write!(f, "{pattern}{op}{n}")
            }
            Self::Exists(pattern) => write!(f, "{pattern}"),
        }
    }
}
