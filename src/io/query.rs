use regex::bytes::{Regex, RegexBuilder};

#[derive(Debug, Clone)]
pub enum QueryExpr {
    Term(String),
    And(Vec<QueryExpr>),
    Or(Vec<QueryExpr>),
    Not(Box<QueryExpr>),
}

pub enum CompiledExpr {
    Term(Regex),
    And(Vec<CompiledExpr>),
    Or(Vec<CompiledExpr>),
    Not(Box<CompiledExpr>),
}

impl CompiledExpr {
    pub fn matches(&self, content: &[u8]) -> bool {
        match self {
            CompiledExpr::Term(re) => re.is_match(content),
            CompiledExpr::And(exprs) => exprs.iter().all(|e| e.matches(content)),
            CompiledExpr::Or(exprs) => exprs.iter().any(|e| e.matches(content)),
            CompiledExpr::Not(e) => !e.matches(content),
        }
    }
}

impl QueryExpr {
    pub fn compile(&self, is_regex: bool, case_sensitive: bool) -> Option<CompiledExpr> {
        match self {
            QueryExpr::Term(t) => {
                let re_str = if is_regex {
                    t.clone()
                } else {
                    regex::escape(t)
                };
                let re = RegexBuilder::new(&re_str)
                    .case_insensitive(!case_sensitive)
                    .build()
                    .ok()?;
                Some(CompiledExpr::Term(re))
            }
            QueryExpr::And(exprs) => {
                let mut compiled = Vec::new();
                for e in exprs {
                    compiled.push(e.compile(is_regex, case_sensitive)?);
                }
                Some(CompiledExpr::And(compiled))
            }
            QueryExpr::Or(exprs) => {
                let mut compiled = Vec::new();
                for e in exprs {
                    compiled.push(e.compile(is_regex, case_sensitive)?);
                }
                Some(CompiledExpr::Or(compiled))
            }
            QueryExpr::Not(e) => Some(CompiledExpr::Not(Box::new(
                e.compile(is_regex, case_sensitive)?,
            ))),
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        let tokens = tokenize(input);
        let mut iter = tokens.into_iter().peekable();
        parse_or(&mut iter)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Token {
    And,
    Or,
    Not,
    LParen,
    RParen,
    Term(String),
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\r' | '\n' => {
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            '"' => {
                chars.next();
                let mut term = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    term.push(c);
                    chars.next();
                }
                tokens.push(Token::Term(term));
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' || c == '"' {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                match s.to_uppercase().as_str() {
                    "AND" => tokens.push(Token::And),
                    "OR" => tokens.push(Token::Or),
                    "NOT" => tokens.push(Token::Not),
                    _ => tokens.push(Token::Term(s)),
                }
            }
        }
    }
    tokens
}

fn parse_or(tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>) -> Option<QueryExpr> {
    let mut left = parse_and(tokens)?;
    while let Some(Token::Or) = tokens.peek() {
        tokens.next();
        let right = parse_and(tokens)?;
        match left {
            QueryExpr::Or(mut exprs) => {
                exprs.push(right);
                left = QueryExpr::Or(exprs);
            }
            _ => {
                left = QueryExpr::Or(vec![left, right]);
            }
        }
    }
    Some(left)
}

fn parse_and(tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>) -> Option<QueryExpr> {
    let mut left = parse_not(tokens)?;
    while let Some(Token::And) = tokens.peek() {
        tokens.next();
        let right = parse_not(tokens)?;
        match left {
            QueryExpr::And(mut exprs) => {
                exprs.push(right);
                left = QueryExpr::And(exprs);
            }
            _ => {
                left = QueryExpr::And(vec![left, right]);
            }
        }
    }
    Some(left)
}

fn parse_not(tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>) -> Option<QueryExpr> {
    if let Some(Token::Not) = tokens.peek() {
        tokens.next();
        let expr = parse_primary(tokens)?;
        Some(QueryExpr::Not(Box::new(expr)))
    } else {
        parse_primary(tokens)
    }
}

fn parse_primary(tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>) -> Option<QueryExpr> {
    match tokens.next()? {
        Token::Term(s) => Some(QueryExpr::Term(s)),
        Token::LParen => {
            let expr = parse_or(tokens)?;
            if let Some(Token::RParen) = tokens.next() {
                Some(expr)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let q = QueryExpr::parse("foo").unwrap();
        if let QueryExpr::Term(t) = q {
            assert_eq!(t, "foo");
        } else {
            panic!("Expected term");
        }
    }

    #[test]
    fn test_parse_and() {
        let q = QueryExpr::parse("foo AND bar").unwrap();
        if let QueryExpr::And(exprs) = q {
            assert_eq!(exprs.len(), 2);
        } else {
            panic!("Expected And");
        }
    }

    #[test]
    fn test_parse_complex() {
        let q = QueryExpr::parse("(foo OR bar) AND NOT baz").unwrap();
        match q {
            QueryExpr::And(exprs) => {
                assert_eq!(exprs.len(), 2);
                match &exprs[0] {
                    QueryExpr::Or(or_exprs) => assert_eq!(or_exprs.len(), 2),
                    _ => panic!("Expected Or"),
                }
                match &exprs[1] {
                    QueryExpr::Not(_) => (),
                    _ => panic!("Expected Not"),
                }
            }
            _ => panic!("Expected And"),
        }
    }

    #[test]
    fn test_parse_quoted() {
        let q = QueryExpr::parse("\"foo AND bar\"").unwrap();
        match q {
            QueryExpr::Term(t) => assert_eq!(t, "foo AND bar"),
            _ => panic!("Expected Term"),
        }
    }

    #[test]
    fn test_matching() {
        let q = QueryExpr::parse("(foo OR bar) AND NOT baz").unwrap();
        let c = q.compile(false, false).unwrap();
        assert!(c.matches(b"foo"));
        assert!(c.matches(b"bar"));
        assert!(!c.matches(b"foo baz"));
        assert!(!c.matches(b"qux"));
    }
}
