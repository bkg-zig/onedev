//! Job match expressions - port of `JobMatch.g4` and the criteria classes of
//! `io.onedev.server.job.match`.
//!
//! They decide which builds may use a job secret (`authorization` of a
//! `JobSecret`, default `on branch "**"`):
//!
//! ```text
//! on branch "main", "release/**"
//! "Project" is "group/**"        "Project" is not "group/sandbox"
//! "Job" is "deploy*"
//! <criteria> and <criteria>      (binds tighter than `or`)
//! <criteria> or <criteria>
//! not (<criteria>)    (<criteria>)
//! ```
//!
//! Several quoted values after `is`/`on branch` are alternatives; after
//! `is not` they are all excluded.

use onedev_build::utils::{match_path, match_string};

/// Field name of the project criteria (`Build.NAME_PROJECT`).
pub const FIELD_PROJECT: &str = "Project";
/// Field name of the job criteria (`Build.NAME_JOB`).
pub const FIELD_JOB: &str = "Job";

/// What an expression is evaluated against.
pub trait MatchContext {
    /// Full path of the project the build belongs to.
    fn project_path(&self) -> &str;
    /// Name of the job (not available when authorising secrets).
    fn job_name(&self) -> Result<&str, String>;
    /// Does the context belong to a branch matching the path pattern?
    /// (`OnBranchCriteria.matches`)
    fn on_branch(&self, pattern: &str) -> Result<bool, String>;
}

/// Which fields an expression may refer to (vanilla `checkField`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Allowed {
    pub project: bool,
    pub job: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobMatch {
    OnBranch(String),
    Project { pattern: String, negate: bool },
    Job { pattern: String, negate: bool },
    And(Vec<JobMatch>),
    Or(Vec<JobMatch>),
    Not(Box<JobMatch>),
}

impl JobMatch {
    /// Parse `text`; `allowed` limits the usable fields.
    pub fn parse(text: &str, allowed: Allowed) -> Result<Self, String> {
        let tokens = tokenize(text)?;
        let mut parser = Parser {
            tokens,
            position: 0,
            allowed,
        };
        let expression = parser.or_expression()?;
        match parser.peek() {
            None => Ok(expression),
            Some(token) => Err(format!("malformed job match: unexpected {token:?}")),
        }
    }

    pub fn matches(&self, context: &dyn MatchContext) -> Result<bool, String> {
        match self {
            JobMatch::OnBranch(pattern) => context.on_branch(pattern),
            JobMatch::Project { pattern, negate } => {
                Ok(match_path(pattern, context.project_path()) != *negate)
            }
            JobMatch::Job { pattern, negate } => Ok(match_string(pattern, context.job_name()?) != *negate),
            JobMatch::And(children) => {
                for child in children {
                    if !child.matches(context)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            JobMatch::Or(children) => {
                for child in children {
                    if child.matches(context)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            JobMatch::Not(inner) => Ok(!inner.matches(context)?),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Quoted(String),
    Open,
    Close,
    Comma,
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
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '"' => {
                chars.next();
                let mut value = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => match chars.next() {
                            Some(escaped) => value.push(escaped),
                            None => return Err("malformed job match: dangling escape".to_owned()),
                        },
                        '"' => {
                            closed = true;
                            break;
                        }
                        other => value.push(other),
                    }
                }
                if !closed {
                    return Err("malformed job match: unterminated quote".to_owned());
                }
                if value.is_empty() {
                    return Err("malformed job match: empty quoted value".to_owned());
                }
                tokens.push(Token::Quoted(value));
            }
            c if c.is_ascii_lowercase() => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_lowercase() {
                        word.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Word(word));
            }
            other => return Err(format!("malformed job match: unexpected `{other}`")),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    allowed: Allowed,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn eat_word(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Token::Word(w)) if w == word) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.peek() == Some(&token) {
            self.position += 1;
            Ok(())
        } else {
            Err(format!("malformed job match: expected {token:?}"))
        }
    }

    fn or_expression(&mut self) -> Result<JobMatch, String> {
        let mut children = vec![self.and_expression()?];
        while self.eat_word("or") {
            children.push(self.and_expression()?);
        }
        Ok(if children.len() == 1 {
            children.remove(0)
        } else {
            JobMatch::Or(children)
        })
    }

    fn and_expression(&mut self) -> Result<JobMatch, String> {
        let mut children = vec![self.unary()?];
        while self.eat_word("and") {
            children.push(self.unary()?);
        }
        Ok(if children.len() == 1 {
            children.remove(0)
        } else {
            JobMatch::And(children)
        })
    }

    fn quoted_list(&mut self) -> Result<Vec<String>, String> {
        let mut values = Vec::new();
        loop {
            match self.peek().cloned() {
                Some(Token::Quoted(value)) => {
                    self.position += 1;
                    values.push(value);
                }
                _ => return Err("malformed job match: expected a quoted value".to_owned()),
            }
            if self.peek() == Some(&Token::Comma) {
                self.position += 1;
            } else {
                return Ok(values);
            }
        }
    }

    fn unary(&mut self) -> Result<JobMatch, String> {
        if self.eat_word("not") {
            self.expect(Token::Open)?;
            let inner = self.or_expression()?;
            self.expect(Token::Close)?;
            return Ok(JobMatch::Not(Box::new(inner)));
        }
        if self.peek() == Some(&Token::Open) {
            self.position += 1;
            let inner = self.or_expression()?;
            self.expect(Token::Close)?;
            return Ok(inner);
        }
        if self.eat_word("on") {
            if !self.eat_word("branch") {
                return Err("malformed job match: expected `on branch`".to_owned());
            }
            let values = self.quoted_list()?;
            return Ok(any_of(values.into_iter().map(JobMatch::OnBranch).collect()));
        }
        let Some(Token::Quoted(field)) = self.peek().cloned() else {
            return Err(format!("malformed job match: unexpected {:?}", self.peek()));
        };
        self.position += 1;
        if !self.eat_word("is") {
            return Err("malformed job match: expected `is` or `is not`".to_owned());
        }
        let negate = self.eat_word("not");
        let values = self.quoted_list()?;
        let make = |pattern: String| -> Result<JobMatch, String> {
            match field.as_str() {
                FIELD_PROJECT if self.allowed.project => Ok(JobMatch::Project { pattern, negate }),
                FIELD_JOB if self.allowed.job => Ok(JobMatch::Job { pattern, negate }),
                FIELD_PROJECT => Err("Project criteria is not supported here".to_owned()),
                FIELD_JOB => Err("Job criteria is not supported here".to_owned()),
                other => Err(format!("Invalid field: {other}")),
            }
        };
        let criteria = values.into_iter().map(make).collect::<Result<Vec<_>, _>>()?;
        // `is not` excludes every value, `is` accepts any of them
        Ok(if negate { all_of(criteria) } else { any_of(criteria) })
    }
}

fn any_of(mut criteria: Vec<JobMatch>) -> JobMatch {
    if criteria.len() == 1 {
        criteria.remove(0)
    } else {
        JobMatch::Or(criteria)
    }
}

fn all_of(mut criteria: Vec<JobMatch>) -> JobMatch {
    if criteria.len() == 1 {
        criteria.remove(0)
    } else {
        JobMatch::And(criteria)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ctx {
        project: &'static str,
        job: Option<&'static str>,
        branches: Vec<&'static str>,
    }

    impl MatchContext for Ctx {
        fn project_path(&self) -> &str {
            self.project
        }
        fn job_name(&self) -> Result<&str, String> {
            self.job.ok_or_else(|| "Job name not available in match context".to_owned())
        }
        fn on_branch(&self, pattern: &str) -> Result<bool, String> {
            Ok(self.branches.iter().any(|branch| match_path(pattern, branch)))
        }
    }

    fn ctx() -> Ctx {
        Ctx {
            project: "group/app",
            job: Some("deploy-prod"),
            branches: vec!["main", "release/1.0"],
        }
    }

    const ALL: Allowed = Allowed { project: true, job: true };

    fn eval(text: &str) -> bool {
        JobMatch::parse(text, ALL)
            .unwrap_or_else(|e| panic!("{text}: {e}"))
            .matches(&ctx())
            .unwrap()
    }

    #[test]
    fn branch_criteria_with_alternatives() {
        assert!(eval(r#"on branch "**""#));
        assert!(eval(r#"on branch "main""#));
        assert!(eval(r#"on branch "dev", "release/*""#));
        assert!(!eval(r#"on branch "dev", "hotfix/**""#));
    }

    #[test]
    fn project_and_job_criteria() {
        assert!(eval(r#""Project" is "group/*""#));
        assert!(!eval(r#""Project" is "other/**""#));
        assert!(eval(r#""Project" is not "group/sandbox", "other/**""#));
        assert!(!eval(r#""Project" is not "group/sandbox", "group/app""#));
        assert!(eval(r#""Job" is "deploy*""#));
        assert!(!eval(r#""Job" is not "deploy*""#));
    }

    #[test]
    fn boolean_composition() {
        assert!(eval(r#"on branch "dev" or on branch "main" and "Job" is "deploy*""#));
        assert!(!eval(r#"on branch "dev" or on branch "main" and "Job" is "build""#));
        assert!(eval(r#"(on branch "dev" or on branch "main") and "Project" is "group/app""#));
        assert!(eval(r#"not (on branch "dev")"#));
        assert!(!eval(r#"not(on branch "main")"#));
    }

    #[test]
    fn missing_job_name_is_an_error_not_a_mismatch() {
        let without_job = Ctx { job: None, ..ctx() };
        let expression = JobMatch::parse(r#""Job" is "x""#, ALL).unwrap();
        assert!(expression.matches(&without_job).is_err());
    }

    #[test]
    fn fields_can_be_restricted() {
        let secrets_only_branches = Allowed::default();
        assert!(JobMatch::parse(r#"on branch "main""#, secrets_only_branches).is_ok());
        assert!(JobMatch::parse(r#""Project" is "a""#, secrets_only_branches)
            .unwrap_err()
            .contains("Project criteria is not supported"));
        assert!(JobMatch::parse(r#""Job" is "a""#, Allowed { project: true, job: false })
            .unwrap_err()
            .contains("Job criteria is not supported"));
        assert!(JobMatch::parse(r#""Other" is "a""#, ALL).unwrap_err().contains("Invalid field"));
    }

    #[test]
    fn malformed_expressions_are_rejected() {
        for bad in [
            "",
            "on",
            "on branch",
            r#"on branch main"#,
            r#"on branch "a","#,
            r#"on branches "a""#,
            r#""Project" "a""#,
            r#""Project" is"#,
            r#"on branch "a" and"#,
            r#"not on branch "a""#,
            r#"(on branch "a""#,
            r#"on branch "a" extra"#,
            r#"on branch """#,
            r#"ON BRANCH "a""#,
        ] {
            assert!(JobMatch::parse(bad, ALL).is_err(), "should be rejected: {bad:?}");
        }
    }
}
