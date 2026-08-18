//! MDX types used by the mdxjs-rs compiler.
//!
//! These were originally defined in the markdown-rs crate. They're extracted
//! here so mdxjs-rs doesn't depend on the old parser.

use crate::line_index::LineIndex;
use core::fmt;

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

/// Byte offset to `Point` lookup; columns and offsets count UTF-16 code units.
pub struct Location<'a> {
    index: LineIndex<'a>,
    len: usize,
}

impl<'a> Location<'a> {
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Location {
            index: LineIndex::from_source(source),
            len: source.len(),
        }
    }

    #[must_use]
    pub fn to_point(&self, offset: usize) -> Option<Point> {
        if offset > self.len {
            return None;
        }
        let byte_offset = u32::try_from(offset).ok()?;
        let mut cursor = self.index.cursor();
        let (line, column) = cursor.offset_to_line_col(byte_offset);
        let utf16_offset = cursor.byte_to_utf16_offset(byte_offset);
        Some(Point::new(
            line as usize,
            column as usize,
            utf16_offset as usize,
        ))
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
