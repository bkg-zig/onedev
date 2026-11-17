//! Safe subset of OneDev's field query language.
//!
//! The complete Java query language is broad. This parser deliberately accepts
//! only conjunctions of exact, quoted comparisons and rejects everything else.
//! It must never be interpreted as SQL.

use crate::{CoreError, CoreResult};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    terms: Vec<Term>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub field: String,
    pub value: String,
}

impl Query {
    pub fn parse(input: Option<&str>) -> CoreResult<Self> {
        let Some(input) = input.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(Self::default());
        };
        let mut terms = Vec::new();
        for part in split_conjunction(input) {
            terms.push(parse_term(part.trim())?);
        }
        Ok(Self { terms })
    }

    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    pub fn value_for(&self, field: &str) -> Option<&str> {
        self.terms
            .iter()
            .find(|term| term.field.eq_ignore_ascii_case(field))
            .map(|term| term.value.as_str())
    }
}

fn parse_term(input: &str) -> CoreResult<Term> {
    let (field, value) = input
        .split_once(" is ")
        .ok_or_else(|| CoreError::validation("only exact 'Field is \"value\"' queries are supported"))?;
    let field = unquote(field.trim());
    let value = unquote(value.trim());
    if field.is_empty() || value.is_empty() {
        return Err(CoreError::validation("query field and value are required"));
    }
    Ok(Term { field, value })
}

fn split_conjunction(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => quoted = !quoted,
            b'a' if !quoted && input[index..].starts_with(" and ") => {
                parts.push(&input[start..index]);
                index += 5;
                start = index;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    parts.push(&input[start..]);
    parts
}

fn unquote(input: &str) -> String {
    input
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(input)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_terms() {
        let query = Query::parse(Some("\"State\" is \"Open\" and \"Priority\" is \"High\""))
            .unwrap();
        assert_eq!(query.terms().len(), 2);
        assert_eq!(query.value_for("state"), Some("Open"));
        assert_eq!(query.value_for("priority"), Some("High"));
    }

    #[test]
    fn rejects_non_exact_expression() {
        assert!(Query::parse(Some("State contains Open")).is_err());
    }
}
