// Imports
use std::{
    error::Error,
    fmt::{
        Display,
        Formatter,
        Result
    }
};


// Custom error
#[derive(Debug)]
pub struct ParseError {
    msg: String,
    pos: Option<(usize, usize)>,
    src: Option<Box<dyn Error>>
}
impl ParseError {
    pub fn new(msg: &str) -> Self {
        Self {
            msg: msg.to_owned(),
            pos: None,
            src: None
        }
    }
    pub fn new_with_pos(msg: &str, pos: (usize, usize)) -> Self {
        Self {
            msg: msg.to_owned(),
            pos: Some(pos),
            src: None
        }
    }
    pub fn new_with_source<E>(msg: &str, src: E) -> Self
        where E: Error + 'static {
        Self {
            msg: msg.to_owned(),
            pos: None,
            src: Some(Box::new(src))
        }
    }
    pub fn new_with_pos_source<E>(msg: &str, pos: (usize, usize), src: E) -> Self
        where E: Error + 'static {
        Self {
            msg: msg.to_owned(),
            pos: Some(pos),
            src: Some(Box::new(src))
        }
    }
}
impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        self.pos.map(|pos| write!(f, "{} <{}:{}>", self.msg, pos.0, pos.1))
                .unwrap_or_else(|| write!(f, "{}", self.msg))
                .and_then(|_| write!(f, "{}", self.source().map_or(String::new(), |src| format!("\n{}", src))))
    }
}
impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.src.as_ref().map(AsRef::as_ref)
    }
}
impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        Self::new(&err.to_string())
    }
}


// Error identifiers
#[derive(Debug, PartialEq)]
pub enum MacroError {
    NotFound(String),
    InfiniteLoop(String)
}


// Tests
#[cfg(test)]
mod tests {
    use super::{ParseError, MacroError};

    #[test]
    fn parse_error() {
        assert_eq!(ParseError::new("simple").to_string(), "simple");
    }

    #[test]
    fn parse_error_with_pos() {
        assert_eq!(ParseError::new_with_pos("error somewhere", (1,2)).to_string(), "error somewhere <1:2>");
    }

    #[test]
    fn parse_error_with_source() {
        assert_eq!(ParseError::new_with_source("error on error", ParseError::new("source")).to_string(), "error on error\nsource");
    }
    #[test]
    fn parse_error_with_pos_and_source() {
        assert_eq!(ParseError::new_with_pos_source("test", (42, 26), ParseError::new("sourcy")).to_string(), "test <42:26>\nsourcy");
    }
    #[test]
    fn compare_macro_errors() {
        assert_ne!(MacroError::InfiniteLoop("".to_owned()), MacroError::NotFound("zzz".to_owned()));
    }
}