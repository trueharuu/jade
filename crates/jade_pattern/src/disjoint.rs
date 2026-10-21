use std::str::FromStr;

use crate::Pattern;
use crate::PatternError;

/// One of two kinds of setup patterns.
#[derive(Clone, Debug, PartialEq)]
pub enum DisjointPattern {
    Mixed(Pattern),
    Disjoint(Pattern, Pattern),
}

impl DisjointPattern {
    #[must_use]
    pub fn join(&self) -> Pattern {
        match self {
            DisjointPattern::Mixed(p) => p.clone(),
            // like idk
            DisjointPattern::Disjoint(p1, p2) => format!("{p1},{p2}")
                .parse()
                .unwrap(),
        }
    }
}

impl FromStr for DisjointPattern {
    type Err = PatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        match parts.len() {
            1 => Ok(DisjointPattern::Mixed(Pattern::from_str(parts[0])?)),
            2 => Ok(DisjointPattern::Disjoint(
                Pattern::from_str(parts[0])?,
                Pattern::from_str(parts[1])?,
            )),
            _ => Err(PatternError::ParseError("disjoint pattern has too many parts".to_string())),
        }
    }
}
