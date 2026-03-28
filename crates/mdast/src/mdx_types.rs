//! MDX types used by the mdxjs-rs compiler.
//!
//! These were originally defined in the markdown-rs crate. They're extracted
//! here so mdxjs-rs doesn't depend on the old parser.

use core::fmt;
use unicode_id::UnicodeID;

/// One place in a source file.
#[derive(Clone, Eq, PartialEq)]
pub struct Point {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Point {
    #[must_use]
    pub fn new(line: usize, column: usize, offset: usize) -> Point {
        Point {
            line,
            column,
            offset,
        }
    }
}

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} ({})", self.line, self.column, self.offset)
    }
}

/// Location of a node in a source file.
#[derive(Clone, Eq, PartialEq)]
pub struct Position {
    pub start: Point,
    pub end: Point,
}

impl Position {
    #[must_use]
    pub fn new(
        start_line: usize,
        start_column: usize,
        start_offset: usize,
        end_line: usize,
        end_column: usize,
        end_offset: usize,
    ) -> Position {
        Position {
            start: Point::new(start_line, start_column, start_offset),
            end: Point::new(end_line, end_column, end_offset),
        }
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}-{}:{} ({}-{})",
            self.start.line,
            self.start.column,
            self.end.line,
            self.end.column,
            self.start.offset,
            self.end.offset
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub place: Option<Box<Place>>,
    pub reason: String,
    pub rule_id: Box<String>,
    pub source: Box<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref place) = self.place {
            write!(f, "{}: ", place)?;
        }
        write!(f, "{} ({}:{})", self.reason, self.source, self.rule_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Position(Position),
    Point(Point),
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Place::Position(p) => write!(
                f,
                "{}:{}-{}:{}",
                p.start.line, p.start.column, p.end.line, p.end.column
            ),
            Place::Point(p) => write!(f, "{}:{}", p.line, p.column),
        }
    }
}

/// Each stop represents a new slice: (relative_offset, absolute_offset).
pub type Stop = (usize, usize);

#[derive(Debug)]
pub struct Location {
    indices: Vec<usize>,
}

impl Location {
    #[must_use]
    pub fn new(bytes: &[u8]) -> Self {
        let mut indices = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'\r' {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    indices.push(index + 2);
                    index += 1;
                } else {
                    indices.push(index + 1);
                }
            } else if bytes[index] == b'\n' {
                indices.push(index + 1);
            }
            index += 1;
        }
        indices.push(index + 1);
        Location { indices }
    }

    #[must_use]
    pub fn to_point(&self, offset: usize) -> Option<Point> {
        if let Some(end) = self.indices.last() {
            if offset < *end {
                let mut index = 0;
                while index < self.indices.len() {
                    if self.indices[index] > offset {
                        break;
                    }
                    index += 1;
                }
                let previous = if index > 0 {
                    self.indices[index - 1]
                } else {
                    0
                };
                return Some(Point::new(index + 1, offset + 1 - previous, offset));
            }
        }
        None
    }

    #[must_use]
    pub fn relative_to_point(&self, stops: &[Stop], relative: usize) -> Option<Point> {
        Self::relative_to_absolute(stops, relative).and_then(|abs| self.to_point(abs))
    }

    #[must_use]
    pub fn relative_to_absolute(stops: &[Stop], relative: usize) -> Option<usize> {
        let mut index = 0;
        while index < stops.len() && stops[index].0 <= relative {
            index += 1;
        }
        if index == 0 {
            None
        } else {
            let (stop_relative, stop_absolute) = &stops[index - 1];
            Some(stop_absolute + (relative - stop_relative))
        }
    }
}

/// Signal used as feedback when parsing MDX ESM/expressions.
#[derive(Clone, Debug)]
pub enum MdxSignal {
    Error(String, usize, Box<String>, Box<String>),
    Eof(String, Box<String>, Box<String>),
    Ok,
}

#[derive(Clone, Debug)]
pub enum MdxExpressionKind {
    Expression,
    AttributeExpression,
    AttributeValueExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributeContent {
    Expression(MdxJsxExpressionAttribute),
    Property(MdxJsxAttribute),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributeValue {
    Expression(AttributeValueExpression),
    Literal(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributeValueExpression {
    pub value: String,
    pub stops: Vec<Stop>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MdxJsxAttribute {
    pub name: String,
    pub value: Option<AttributeValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MdxJsxExpressionAttribute {
    pub value: String,
    pub stops: Vec<Stop>,
}

#[must_use]
pub fn id_start(char: char) -> bool {
    UnicodeID::is_id_start(char) || matches!(char, '$' | '_')
}

#[must_use]
pub fn id_cont(char: char, jsx: bool) -> bool {
    UnicodeID::is_id_continue(char)
        || matches!(char, '\u{200c}' | '\u{200d}')
        || (jsx && char == '-')
}

/// Make a value safe for injection as a URL.
#[must_use]
pub fn sanitize_uri(value: &str) -> String {
    encode_uri(&normalize_uri(value), true)
}

fn encode_uri(value: &str, encode_html: bool) -> String {
    let mut result = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut start = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == 0 || (encode_html && matches!(byte, b'&' | b'"' | b'<' | b'>')) {
            result.push_str(&value[start..index]);
            result.push_str(match byte {
                b'\0' => "\u{FFFD}",
                b'&' => "&amp;",
                b'"' => "&quot;",
                b'<' => "&lt;",
                _ => "&gt;",
            });
            start = index + 1;
        }
        index += 1;
    }
    result.push_str(&value[start..]);
    result
}

fn normalize_uri(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut result = String::with_capacity(value.len());
    let mut index = 0;
    let mut start = 0;
    let mut buff = [0u8; 4];
    while index < chars.len() {
        let ch = chars[index];
        if ch == '%'
            && index + 2 < chars.len()
            && chars[index + 1].is_ascii_alphanumeric()
            && chars[index + 2].is_ascii_alphanumeric()
        {
            index += 3;
            continue;
        }
        if ch >= '\u{0080}'
            || !matches!(ch, '!' | '#' | '$' | '&'..=';' | '=' | '?'..='Z' | '_' | 'a'..='z' | '~')
        {
            let s: String = chars[start..index].iter().collect();
            result.push_str(&s);
            ch.encode_utf8(&mut buff);
            for &b in &buff[..ch.len_utf8()] {
                result.push_str(&format!("%{:>02X}", b));
            }
            start = index + 1;
        }
        index += 1;
    }
    let s: String = chars[start..].iter().collect();
    result.push_str(&s);
    result
}
