//! Job retry conditions - port of `RetryCondition.g4` and the criteria
//! classes of `io.onedev.server.buildspec.job.retrycondition`.
//!
//! ```text
//! never
//! timed out
//! "Log" contains "ConnectException|Connection reset"
//! "env" is "prod"  /  "env" is not "prod"  /  "env" is empty  /  "env" is not empty
//! <criteria> and <criteria>   (binds tighter than `or`)
//! <criteria> or <criteria>
//! not (<criteria>)   /   (<criteria>)
//! ```

use std::collections::BTreeMap;

use regex::Regex;

/// What a retry decision can look at.
#[derive(Debug, Clone)]
pub struct RetryContext<'a> {
    pub timed_out: bool,
    /// Message of the failure that ended the attempt.
    pub error_message: Option<&'a str>,
    /// Build log text of the attempt.
    pub log: &'a str,
    /// Parameter values of the build (a parameter can have several).
    pub params: &'a BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
enum Criteria {
    TimedOut,
    ParamEmpty {
        name: String,
        empty: bool,
    },
    Param {
        name: String,
        value: String,
        negate: bool,
    },
    LogContains(Regex),
    And(Box<Criteria>, Box<Criteria>),
    Or(Box<Criteria>, Box<Criteria>),
    Not(Box<Criteria>),
}

/// A parsed `retryCondition`.
#[derive(Debug, Clone)]
pub struct RetryCondition {
    /// `None` for `never`.
    criteria: Option<Criteria>,
}

/// The name of the log pseudo field (`Build.NAME_LOG`).
pub const FIELD_LOG: &str = "Log";

impl RetryCondition {
    pub fn never() -> Self {
        Self { criteria: None }
    }

    pub fn is_never(&self) -> bool {
        self.criteria.is_none()
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let tokens = tokenize(text)?;
        let mut parser = Parser {
            tokens,
            position: 0,
        };
        if parser.peek_word("never") {
            parser.position += 1;
            parser.expect_end()?;
            return Ok(Self::never());
        }
        let criteria = parser.or_expression()?;
        parser.expect_end()?;
        Ok(Self {
            criteria: Some(criteria),
        })
    }

    pub fn matches(&self, context: &RetryContext<'_>) -> bool {
        match &self.criteria {
            None => false,
            Some(criteria) => evaluate(criteria, context),
        }
    }

    /// Names of the parameters the condition refers to.
    pub fn param_names(&self) -> Vec<String> {
        fn collect(criteria: &Criteria, out: &mut Vec<String>) {
            match criteria {
                Criteria::ParamEmpty { name, .. } | Criteria::Param { name, .. } => {
                    out.push(name.clone())
                }
                Criteria::And(a, b) | Criteria::Or(a, b) => {
                    collect(a, out);
                    collect(b, out);
                }
                Criteria::Not(inner) => collect(inner, out),
                Criteria::TimedOut | Criteria::LogContains(_) => {}
            }
        }
        let mut names = Vec::new();
        if let Some(criteria) = &self.criteria {
            collect(criteria, &mut names);
        }
        names
    }
}

fn evaluate(criteria: &Criteria, context: &RetryContext<'_>) -> bool {
    match criteria {
        Criteria::TimedOut => context.timed_out,
        Criteria::ParamEmpty { name, empty } => {
            let is_empty = context
                .params
                .get(name)
                .is_none_or(|values| values.iter().all(|value| value.is_empty()));
            is_empty == *empty
        }
        Criteria::Param {
            name,
            value,
            negate,
        } => {
            let contains = context
                .params
                .get(name)
                .is_some_and(|values| values.contains(value));
            contains != *negate
        }
        Criteria::LogContains(pattern) => {
            context
                .error_message
                .is_some_and(|message| pattern.is_match(message))
                || pattern.is_match(context.log)
        }
        Criteria::And(a, b) => evaluate(a, context) && evaluate(b, context),
        Criteria::Or(a, b) => evaluate(a, context) || evaluate(b, context),
        Criteria::Not(inner) => !evaluate(inner, context),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Quoted(String),
    Open,
    Close,
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(Token::Open);
            }
            ')' => {
                chars.next();
                tokens.push(Token::Close);
            }
            '"' => {
                chars.next();
                let mut value = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => match chars.next() {
                            Some(escaped) => value.push(escaped),
                            None => {
                                return Err("malformed retry condition: dangling escape".to_owned());
                            }
                        },
                        '"' => {
                            closed = true;
                            break;
                        }
                        other => value.push(other),
                    }
                }
                if !closed {
                    return Err("malformed retry condition: unterminated quote".to_owned());
                }
                if value.is_empty() {
                    return Err("malformed retry condition: empty quoted value".to_owned());
                }
                tokens.push(Token::Quoted(value));
            }
            c if c.is_ascii_alphabetic() => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphabetic() {
                        word.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Word(word.to_ascii_lowercase()));
            }
            other => return Err(format!("malformed retry condition: unexpected `{other}`")),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn peek_word(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Token::Word(w)) if w == word)
    }

    fn eat_word(&mut self, word: &str) -> bool {
        if self.peek_word(word) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect_end(&self) -> Result<(), String> {
        match self.peek() {
            None => Ok(()),
            Some(token) => Err(format!("malformed retry condition: unexpected {token:?}")),
        }
    }

    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.peek() == Some(&token) {
            self.position += 1;
            Ok(())
        } else {
            Err(format!("malformed retry condition: expected {token:?}"))
        }
    }

    fn or_expression(&mut self) -> Result<Criteria, String> {
        let mut left = self.and_expression()?;
        while self.eat_word("or") {
            let right = self.and_expression()?;
            left = Criteria::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and_expression(&mut self) -> Result<Criteria, String> {
        let mut left = self.unary()?;
        while self.eat_word("and") {
            let right = self.unary()?;
            left = Criteria::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Criteria, String> {
        if self.eat_word("not") {
            self.expect(Token::Open)?;
            let inner = self.or_expression()?;
            self.expect(Token::Close)?;
            return Ok(Criteria::Not(Box::new(inner)));
        }
        if self.peek() == Some(&Token::Open) {
            self.position += 1;
            let inner = self.or_expression()?;
            self.expect(Token::Close)?;
            return Ok(inner);
        }
        if self.eat_word("timed") {
            if !self.eat_word("out") {
                return Err("malformed retry condition: expected `timed out`".to_owned());
            }
            return Ok(Criteria::TimedOut);
        }
        let Some(Token::Quoted(field)) = self.peek().cloned() else {
            return Err(format!(
                "malformed retry condition: unexpected {:?}",
                self.peek()
            ));
        };
        self.position += 1;

        if self.eat_word("contains") {
            let Some(Token::Quoted(value)) = self.peek().cloned() else {
                return Err("malformed retry condition: expected a quoted value".to_owned());
            };
            self.position += 1;
            if field != FIELD_LOG {
                return Err(format!(
                    "field \"{field}\" does not support `contains` (only \"{FIELD_LOG}\" does)"
                ));
            }
            let pattern =
                Regex::new(&value).map_err(|e| format!("invalid log pattern \"{value}\": {e}"))?;
            return Ok(Criteria::LogContains(pattern));
        }
        if !self.eat_word("is") {
            return Err("malformed retry condition: expected `is` or `contains`".to_owned());
        }
        let negate = self.eat_word("not");
        if self.eat_word("empty") {
            return Ok(Criteria::ParamEmpty {
                name: field,
                empty: !negate,
            });
        }
        let Some(Token::Quoted(value)) = self.peek().cloned() else {
            return Err("malformed retry condition: expected a quoted value or `empty`".to_owned());
        };
        self.position += 1;
        if field == FIELD_LOG {
            return Err(format!("field \"{FIELD_LOG}\" only supports `contains`"));
        }
        Ok(Criteria::Param {
            name: field,
            value,
            negate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect()))
            .collect()
    }

    fn check(
        condition: &str,
        timed_out: bool,
        error: Option<&str>,
        log: &str,
        p: &BTreeMap<String, Vec<String>>,
    ) -> bool {
        RetryCondition::parse(condition)
            .unwrap_or_else(|e| panic!("{condition}: {e}"))
            .matches(&RetryContext {
                timed_out,
                error_message: error,
                log,
                params: p,
            })
    }

    #[test]
    fn never_and_timed_out() {
        let none = BTreeMap::new();
        let never = RetryCondition::parse("  never ").unwrap();
        assert!(never.is_never());
        assert!(!never.matches(&RetryContext {
            timed_out: true,
            error_message: Some("x"),
            log: "x",
            params: &none
        }));
        assert!(check("timed out", true, None, "", &none));
        assert!(!check("timed out", false, Some("boom"), "", &none));
    }

    #[test]
    fn log_conditions_use_regexes_on_error_message_and_log() {
        let none = BTreeMap::new();
        let cond = r#""Log" contains "Connection (reset|refused)""#;
        assert!(check(
            cond,
            false,
            None,
            "... Connection reset by peer ...",
            &none
        ));
        assert!(check(cond, false, Some("Connection refused"), "", &none));
        assert!(!check(cond, false, Some("other"), "all fine", &none));
        // escaped quotes inside the pattern
        assert!(check(
            r#""Log" contains "say \"hi\"""#,
            false,
            None,
            r#"say "hi""#,
            &none
        ));
    }

    #[test]
    fn param_conditions() {
        let p = params(&[("env", &["prod", "staging"]), ("empty", &[""])]);
        assert!(check(r#""env" is "prod""#, false, None, "", &p));
        assert!(!check(r#""env" is "dev""#, false, None, "", &p));
        assert!(check(r#""env" is not "dev""#, false, None, "", &p));
        assert!(check(r#""missing" is empty"#, false, None, "", &p));
        assert!(check(r#""empty" is empty"#, false, None, "", &p));
        assert!(check(r#""env" is not empty"#, false, None, "", &p));
        assert!(!check(r#""env" is empty"#, false, None, "", &p));
    }

    #[test]
    fn boolean_composition_and_precedence() {
        let p = params(&[("env", &["prod"])]);
        // `and` binds tighter than `or`: a or (b and c)
        assert!(check(
            r#"timed out or "env" is "prod" and "env" is "dev""#,
            true,
            None,
            "",
            &p
        ));
        assert!(!check(
            r#"timed out or "env" is "prod" and "env" is "dev""#,
            false,
            None,
            "",
            &p
        ));
        assert!(check(
            r#"(timed out or "env" is "prod") and "env" is not "dev""#,
            false,
            None,
            "",
            &p
        ));
        assert!(check(r#"not ("env" is "dev")"#, false, None, "", &p));
        assert!(!check(
            r#"not (timed out or "env" is "prod")"#,
            false,
            None,
            "",
            &p
        ));
        assert_eq!(
            RetryCondition::parse(r#""a" is "1" and "b" is empty or not ("c" is "x")"#)
                .unwrap()
                .param_names(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn malformed_conditions_are_rejected() {
        for bad in [
            "",
            "always",
            "never never",
            "timed",
            r#""Log" is "x""#,
            r#""env" contains "x""#,
            r#""Log" contains "(unclosed""#,
            r#""env" is"#,
            r#""env" is "x" and"#,
            r#"not "env" is "x""#,
            r#"("env" is "x""#,
            r#""unterminated"#,
            r#""" is "x""#,
            "timed out extra",
        ] {
            assert!(
                RetryCondition::parse(bad).is_err(),
                "should be rejected: {bad:?}"
            );
        }
    }
}
