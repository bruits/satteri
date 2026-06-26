use memchr::memchr;

use oxc_allocator::Allocator;
use oxc_ast::ast::{BigIntLiteral, NumericLiteral};
use oxc_ast_visit::Visit;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;

use crate::{
    firstpass::FirstPass,
    parse::{Item, ItemBody},
    strings::CowStr,
};

/// Max mutual-recursion depth between `scan_mdx_expression_end_inner` and
/// `scan_mdx_jsx_tag_end_inner` for inputs like `<a {<b {<c …}/>}/>`. Bounds
/// the parser stack; once exceeded the scanner returns `None` and the `<` or
/// `{` is left for the caller to handle as a parse error.
const MAX_MDX_NESTING: u32 = 32;

/// Strip the micromark `indentSize = 2` prefix from each continuation
/// line of an MDX expression. Matches `micromark-factory-mdx-expression`
/// which consumes up to 2 columns of whitespace after a line ending
/// (tabs expand to the next multiple of 4; any leftover tab columns are
/// emitted as literal spaces).
///
/// `container_content_col` is the 1-indexed column where the innermost
/// list/blockquote's content begins — continuation lines are conceptually
/// at that column, which affects tab-stop math when a tab straddles the
/// container prefix boundary.
pub(crate) fn dedent_expression_continuation(
    s: &str,
    container_content_col: usize,
) -> alloc::borrow::Cow<'_, str> {
    if !s.contains('\n') && !s.contains('\r') {
        return alloc::borrow::Cow::Borrowed(s);
    }
    const INDENT: usize = 2;
    const TAB_SIZE: usize = 4;
    let base_col = if container_content_col == 0 {
        1
    } else {
        container_content_col
    };
    let bytes = s.as_bytes();
    let mut out = alloc::string::String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
        i += 1;
    }
    out.push_str(&s[..i]);
    while i < bytes.len() {
        let line_end_start = i;
        if bytes[i] == b'\r' {
            i += 1;
            if i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'\n' {
            i += 1;
        } else {
            break;
        }
        out.push_str(&s[line_end_start..i]);
        // Strip up to INDENT columns of leading whitespace from the new line.
        // Track `column` starting from `base_col - 1` so tab-stop math runs
        // against the true absolute column in the source.
        let mut stripped = 0usize;
        let mut column = base_col - 1;
        while i < bytes.len() && stripped < INDENT {
            let b = bytes[i];
            if b == b' ' {
                stripped += 1;
                column += 1;
                i += 1;
            } else if b == b'\t' {
                let next_col = (column / TAB_SIZE + 1) * TAB_SIZE;
                let tab_width = next_col - column;
                let to_strip = (INDENT - stripped).min(tab_width);
                stripped += to_strip;
                for _ in 0..(tab_width - to_strip) {
                    out.push(PHANTOM_SPACE);
                }
                column = next_col;
                i += 1;
            } else {
                break;
            }
        }
        let rest_start = i;
        while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
            i += 1;
        }
        out.push_str(&s[rest_start..i]);
    }
    alloc::borrow::Cow::Owned(out)
}

/// Sentinel marking a "phantom space": a column of dedent-produced whitespace
/// with no underlying source byte (e.g. leftover of a partial-tab consume).
/// A plain space can't be the marker because the oxc path then can't tell
/// dedent whitespace from authored whitespace, and the dedent bleeds into
/// template-literal values. Consumers substitute it back to a space or strip
/// it (see the strip sites in `satteri-mdxjs-rs::hast_util_to_oxc`).
pub(crate) const PHANTOM_SPACE: char = '\u{F002}';

/// Mirror `mdast-util-mdx-jsx`'s attribute-expression continuation-line
/// handling.
///
/// `container_content_col` is the source column of the first char *after*
/// list/blockquote prefix stripping. `extra_strip_cols` is whatever the
/// surrounding constructs (notably container directives' `initialSize`)
/// strip on top of the standard `indentSize = 2` expression dedent.
fn strip_expression_indent(
    s: &str,
    container_content_col: usize,
    extra_strip_cols: usize,
) -> alloc::string::String {
    const INDENT_SIZE: usize = 2;
    const TAB_WIDTH: usize = 4;
    let base_col = if container_content_col == 0 {
        1
    } else {
        container_content_col
    };
    let strip_cols = INDENT_SIZE + extra_strip_cols;
    let mut result = alloc::string::String::with_capacity(s.len());
    let mut at_line_start = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\n' {
            result.push('\n');
            i += 1;
            at_line_start = true;
            continue;
        }
        if c == b'\r' {
            result.push('\r');
            i += 1;
            if i < bytes.len() && bytes[i] == b'\n' {
                result.push('\n');
                i += 1;
            }
            at_line_start = true;
            continue;
        }
        if !at_line_start {
            let ch_len = char_len_utf8(c);
            result.push_str(&s[i..i + ch_len]);
            i += ch_len;
            continue;
        }
        // At line start: strip up to `strip_cols` columns of whitespace,
        // treating the first byte of the line as being at absolute column
        // `base_col` in the source (which affects tab-stop math).
        let mut cols_consumed = 0usize;
        let mut col = base_col;
        while i < bytes.len() && cols_consumed < strip_cols {
            match bytes[i] {
                b' ' => {
                    cols_consumed += 1;
                    col += 1;
                    i += 1;
                }
                b'\t' => {
                    let tab_cols = TAB_WIDTH - ((col - 1) % TAB_WIDTH);
                    let want = strip_cols - cols_consumed;
                    if want >= tab_cols {
                        cols_consumed += tab_cols;
                        col += tab_cols;
                        i += 1;
                    } else {
                        // Partial tab consumption: emit phantom-space sentinels
                        // for each column still owed of the tab (see
                        // `PHANTOM_SPACE`). Real space would bleed into
                        // template-literal cooked values via oxc.
                        let keep_cols = tab_cols - want;
                        for _ in 0..keep_cols {
                            result.push(PHANTOM_SPACE);
                        }
                        i += 1;
                        break;
                    }
                }
                _ => break,
            }
        }
        at_line_start = false;
    }
    result
}

fn strip_attr_continuation_indent(s: &str) -> alloc::borrow::Cow<'_, str> {
    if !s.contains('\n') && !s.contains('\r') {
        return alloc::borrow::Cow::Borrowed(s);
    }
    let mut result = alloc::string::String::with_capacity(s.len());
    let mut at_line_start = false;
    for c in s.chars() {
        if c == '\n' || c == '\r' {
            result.push(c);
            at_line_start = true;
        } else if at_line_start && (c == ' ' || c == '\t') {
            continue;
        } else {
            at_line_start = false;
            result.push(c);
        }
    }
    alloc::borrow::Cow::Owned(result)
}

use crate::utils::decode_html_entities as decode_attr_entities;

fn is_mdx_unicode_whitespace(s: &[u8], ix: usize) -> bool {
    let b = s[ix];
    if b.is_ascii_whitespace() {
        return true;
    }
    // Non-ASCII path: validate only the bytes for the current code point
    // (at most 4 bytes). Validating the rest of the document via
    // `from_utf8` was a hot-path cost in the JSX scanner.
    if b < 0x80 {
        return false;
    }
    let len = char_len_utf8(b);
    let end = ix + len;
    if end > s.len() {
        return false;
    }
    let Ok(text) = core::str::from_utf8(&s[ix..end]) else {
        return false;
    };
    let c = text.chars().next().unwrap();
    // Unicode Zs category, BOM (U+FEFF)
    matches!(
        c,
        '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

fn char_len_utf8(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xFF => 4,
        _ => 1,
    }
}

fn decode_utf8_char(s: &[u8], ix: usize) -> Option<char> {
    core::str::from_utf8(&s[ix..]).ok()?.chars().next()
}

fn is_jsx_name_start(s: &[u8], ix: usize) -> bool {
    let b = s[ix];
    if b < 0x80 {
        return b.is_ascii_alphabetic() || b == b'_' || b == b'$';
    }
    decode_utf8_char(s, ix).is_some_and(unicode_id_start::is_id_start)
}

fn is_jsx_name_continue(s: &[u8], ix: usize) -> bool {
    let b = s[ix];
    if b < 0x80 {
        // `.` and `:` are member/namespace separators handled at a higher
        // level in the tag-name scanner; treating them as continuation chars
        // here lets garbage like `<a..b>` or `<a:b.c>` through, which
        // micromark-extension-mdx-jsx rejects.
        return b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'$');
    }
    decode_utf8_char(s, ix).is_some_and(unicode_id_start::is_id_continue)
}

/// Result of trying to parse an ESM block with oxc.
pub(crate) enum EsmParseResult {
    Complete,
    Incomplete,
    Error,
}

/// Validate an MDX expression body as JS via oxc, mirroring
/// `acorn.parseExpressionAt` in mdx-js. Wraps as `(body)` so `{}/m` reads
/// as `{} / m` and multi-statement bodies (`{a;b}`) get rejected. Falls
/// back to a manual scan for comment-only bodies since `(/* foo */)` is
/// itself invalid. Returns `(offset, detail)` for the first error.
pub(crate) fn try_parse_expression_body(
    value: &str,
    allocator: &mut Allocator,
) -> Option<(usize, String)> {
    let source_type = SourceType::mjs().with_jsx(true);

    // Empty / whitespace-only bodies (`{}`, `{ }`, `{\n}`) are valid per
    // mdx-js's allowEmpty.
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Primary check: parse the body in expression context by wrapping it
    // in parens. mdx-js uses acorn `parseExpressionAt`, which rejects
    // multi-statement bodies like `{a;b}` or `{y\n a}` even though both
    // would parse as a program. The wrap also forces `{}/m` to be read as
    // an empty-object division rather than a block + unterminated regex.
    allocator.reset();
    let wrapped = alloc::format!("({value})");
    let source = allocator.alloc_str(&wrapped);
    let ret = Parser::new(allocator, source, source_type)
        .with_options(ParseOptions::default())
        .parse();
    if ret.errors.is_empty() {
        // oxc accepts legacy octal literals (`01`, `09`, `0123`) even in
        // .mjs / module sources. acorn (used by mdx-js) rejects them as
        // "Invalid number" / "Octal literals are not allowed in strict
        // mode" in any expression context. Walk the AST to surface them.
        let mut finder = LegacyOctalFinder::default();
        finder.visit_program(&ret.program);
        if let Some(offset) = finder.offset {
            // Span is in wrapped coords; subtract 1 for the leading `(`.
            return Some((offset.saturating_sub(1), "Invalid number".to_string()));
        }
        return None;
    }

    // Fallback: comment-only bodies (`{/* foo */}`) trip the wrapped
    // parser since `(/* foo */)` is invalid. Walk past comments and
    // accept if nothing non-whitespace remains.
    let bytes = value.as_bytes();
    let mut i = 0;
    let mut has_non_ws = false;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let comment_start = i;
            i += 2;
            let mut closed = false;
            while i + 1 < bytes.len() {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    closed = true;
                    i += 2;
                    break;
                }
                i += 1;
            }
            if !closed {
                return Some((comment_start, "Unterminated block comment".to_string()));
            }
        } else {
            if !matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
                has_non_ws = true;
            }
            i += 1;
        }
    }
    if !has_non_ws {
        return None;
    }

    let first = ret.errors.first()?;
    let err_offset = first
        .labels
        .as_ref()
        .and_then(|labels| labels.first().map(|l| l.offset()))
        // The parens wrap shifts offsets by 1; subtract to map back to
        // body coordinates.
        .map(|o| o.saturating_sub(1))
        .unwrap_or(value.len());
    Some((err_offset, first.message.to_string()))
}

/// Validate a JS expression body via oxc, returning the parse error's byte
/// offset *within `body`* plus a detail message, or `None` if it parses.
///
/// `body` may carry [`PHANTOM_SPACE`] sentinels: they are stripped before oxc
/// sees them (it rejects the private-use code point) and the reported offset is
/// mapped back through them, so the result stays in `body`'s own coordinates.
pub(crate) fn validate_expression_body(
    body: &str,
    allocator: &mut Allocator,
) -> Option<(usize, alloc::string::String)> {
    if !body.contains(PHANTOM_SPACE) {
        return try_parse_expression_body(body, allocator);
    }
    let clean = body.replace(PHANTOM_SPACE, "");
    let (clean_offset, detail) = try_parse_expression_body(&clean, allocator)?;
    // Re-walk `body`, skipping phantoms, until the cleaned position reaches the
    // error; that byte index is the equivalent offset in `body`.
    let mut cleaned = 0;
    for (idx, ch) in body.char_indices() {
        if cleaned >= clean_offset {
            return Some((idx, detail));
        }
        if ch != PHANTOM_SPACE {
            cleaned += ch.len_utf8();
        }
    }
    Some((body.len(), detail))
}

/// Walks an oxc AST looking for numeric literals whose raw text starts with
/// a `0` followed by another digit — i.e. legacy octals (`01`, `0123`) or
/// "non-octal decimal" literals (`08`, `09`). acorn rejects both in strict
/// mode; oxc accepts both even in `.mjs` source.
#[derive(Default)]
struct LegacyOctalFinder {
    offset: Option<usize>,
}

impl LegacyOctalFinder {
    fn check_raw(&mut self, raw: Option<&str>, span_start: u32) {
        if self.offset.is_some() {
            return;
        }
        let raw = match raw {
            Some(r) => r.as_bytes(),
            None => return,
        };
        if raw.len() >= 2 && raw[0] == b'0' && raw[1].is_ascii_digit() {
            self.offset = Some(span_start as usize);
        }
    }
}

impl<'a> Visit<'a> for LegacyOctalFinder {
    fn visit_numeric_literal(&mut self, lit: &NumericLiteral<'a>) {
        self.check_raw(lit.raw.as_deref(), lit.span.start);
    }

    fn visit_big_int_literal(&mut self, lit: &BigIntLiteral<'a>) {
        // `01n`, `0o7n` etc. — only legacy-octal-looking BigInts are an
        // error; `0n` and `0x1n` / `0o1n` / `0b1n` are valid.
        self.check_raw(lit.raw.as_deref(), lit.span.start);
    }
}

pub(crate) fn try_parse_esm(value: &str, allocator: &mut Allocator) -> EsmParseResult {
    allocator.reset();
    let source_type = SourceType::mjs().with_jsx(true);
    let source = allocator.alloc_str(value);
    let ret = Parser::new(allocator, source, source_type)
        .with_options(ParseOptions::default())
        .parse();

    if ret.errors.is_empty() {
        return EsmParseResult::Complete;
    }

    let error = &ret.errors[0];
    let error_offset = error
        .labels
        .as_ref()
        .and_then(|labels| labels.first().map(|l| l.offset()))
        .unwrap_or(value.len());

    if error_offset >= value.len() {
        EsmParseResult::Incomplete
    } else {
        EsmParseResult::Error
    }
}

/// Keywords after which `/` starts a regex literal, not division.
const REGEX_KEYWORDS: &[&[u8]] = &[
    b"await",
    b"case",
    b"delete",
    b"in",
    b"instanceof",
    b"new",
    b"of",
    b"return",
    b"throw",
    b"typeof",
    b"void",
    b"yield",
];

/// Determine whether `/` at `pos` is the start of a regex literal.
fn slash_is_regex(bytes: &[u8], pos: usize) -> bool {
    let mut i = pos;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' => continue,
            b')' | b']' => return false,
            b'"' | b'\'' | b'`' => return false,
            b'+' if i > 0 && bytes[i - 1] == b'+' => return false,
            b'-' if i > 0 && bytes[i - 1] == b'-' => return false,
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$' => {
                let end = i + 1;
                while i > 0
                    && (bytes[i - 1].is_ascii_alphanumeric()
                        || bytes[i - 1] == b'_'
                        || bytes[i - 1] == b'$')
                {
                    i -= 1;
                }
                let word = &bytes[i..end];
                let is_keyword_boundary = i == 0
                    || (!bytes[i - 1].is_ascii_alphanumeric()
                        && bytes[i - 1] != b'_'
                        && bytes[i - 1] != b'$');
                if is_keyword_boundary && REGEX_KEYWORDS.contains(&word) {
                    return true;
                }
                return false;
            }
            _ => return true,
        }
    }
    true
}

/// Scan a regex literal starting at `/`, returning the offset past the flags.
fn scan_regex(bytes: &[u8], start: usize) -> usize {
    let mut ix = start + 1;
    while ix < bytes.len() {
        match bytes[ix] {
            b'/' => {
                ix += 1;
                while ix < bytes.len() && bytes[ix].is_ascii_alphanumeric() {
                    ix += 1;
                }
                return ix;
            }
            b'\\' => ix += 2,
            b'[' => {
                ix += 1;
                while ix < bytes.len() && bytes[ix] != b']' {
                    if bytes[ix] == b'\\' {
                        ix += 1;
                    }
                    ix += 1;
                }
                if ix < bytes.len() {
                    ix += 1;
                }
            }
            b'\n' | b'\r' => return ix,
            _ => ix += 1,
        }
    }
    ix
}

/// Whether a `/` at `ix` opens a regex literal rather than being division,
/// given whether the previous token produced a value. Shared by the ESM-block
/// scan and the expression scan so the two heuristics can't drift apart.
///
/// `slash_is_regex` alone mis-reads `/` after a regex close (`/x/ /y/`) or after
/// a `}` (object-literal close, `{}/_`) as a new regex. `prev_was_value` forces
/// division there, but only when the prior non-space byte isn't an identifier
/// char so a regex-introducing keyword (`return /re/`) still reads as a regex.
fn slash_is_regex_after(bytes: &[u8], ix: usize, prev_was_value: bool) -> bool {
    let prev_is_ident_char = {
        let mut j = ix;
        while j > 0 && matches!(bytes[j - 1], b' ' | b'\t') {
            j -= 1;
        }
        j > 0
            && (bytes[j - 1].is_ascii_alphanumeric()
                || bytes[j - 1] == b'_'
                || bytes[j - 1] == b'$')
    };
    let force_division = prev_was_value && !prev_is_ident_char;
    !force_division && slash_is_regex(bytes, ix)
}

/// Skip a `'…'` or `"…"` string whose opening quote is at `start`, returning the
/// offset past the closing quote. A string can't span a newline in JS, so the
/// scan stops at one; a backslash escapes the next byte. Shared by the ESM-block
/// scan and the expression scan.
fn skip_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let len = bytes.len();
    let mut ix = start + 1;
    while ix < len && bytes[ix] != quote && bytes[ix] != b'\n' && bytes[ix] != b'\r' {
        if bytes[ix] == b'\\' {
            ix += 1;
        }
        ix += 1;
    }
    if ix < len && bytes[ix] == quote {
        ix += 1;
    }
    ix
}

/// Is the byte at `ix` a line ending followed by a blank line (or EOF)?
///
/// Used to cut the expression scan at block boundaries, so an unclosed `{`
/// can't silently consume subsequent blocks.
fn is_blank_line_next(bytes: &[u8], ix: usize) -> bool {
    let mut j = ix + 1;
    if bytes[ix] == b'\r' && j < bytes.len() && bytes[j] == b'\n' {
        j += 1;
    }
    let mut k = j;
    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
        k += 1;
    }
    k >= bytes.len() || bytes[k] == b'\n' || bytes[k] == b'\r'
}

/// Checks whether a continuation line after a newline has a valid container
/// prefix. Returns the number of prefix bytes to skip, or `None` to reject
/// the line as a lazy continuation.
pub(crate) type ContainerLineCheck<'a> = &'a dyn Fn(&[u8]) -> Option<usize>;

/// Outcome of a per-newline container prefix check inside an inline scan.
#[derive(Copy, Clone, Eq, PartialEq)]
enum LineMode {
    /// Container prefix matched: continuation line is fully part of the block.
    Strict,
    /// Container prefix missing: line is a lazy paragraph continuation. Inline
    /// expressions only allow the closing `}` (and surrounding whitespace) on
    /// such a line — any body content here is rejected by the caller.
    Lazy,
}

/// After advancing past a newline, check the container prefix on the new line.
/// Returns `Some(())` if OK (and advances `ix`), or `None` to reject.
fn check_container_after_newline(
    bytes: &[u8],
    ix: &mut usize,
    container_check: &Option<ContainerLineCheck<'_>>,
) -> Option<()> {
    if let Some(check) = container_check {
        if *ix < bytes.len() {
            if let Some(skip) = check(&bytes[*ix..]) {
                *ix += skip;
            } else {
                return None;
            }
        }
    }
    Some(())
}

/// Like `check_container_after_newline`, but on a missing prefix accepts the
/// line as lazy (no skip) and reports the mode back to the caller. Used by
/// inline expression scans where the enclosing paragraph allows lazy
/// continuation but body content on the lazy line must be rejected.
fn check_container_after_newline_lazy(
    bytes: &[u8],
    ix: &mut usize,
    container_check: &Option<ContainerLineCheck<'_>>,
) -> LineMode {
    if let Some(check) = container_check {
        if *ix < bytes.len() {
            if let Some(skip) = check(&bytes[*ix..]) {
                *ix += skip;
                return LineMode::Strict;
            }
            return LineMode::Lazy;
        }
    }
    LineMode::Strict
}

fn scan_mdx_expression_end(bytes: &[u8], inline: bool) -> Option<usize> {
    scan_mdx_expression_end_inner(bytes, inline, None, false, true, 0)
}

/// Scan an MDX expression `{...}`, finding the matching closing `}`.
///
/// Uses a lightweight JS lexer that properly handles strings, comments,
/// template literals, and regex literals.
///
/// When `inline` is true, blank lines abort the scan.
///
/// When `container_check` is provided, each continuation line is validated
/// through the closure. If it returns `None`, the line is rejected as a lazy
/// continuation. Otherwise the returned number of prefix bytes are skipped.
fn scan_mdx_expression_end_inner(
    bytes: &[u8],
    inline: bool,
    container_check: Option<ContainerLineCheck<'_>>,
    // When true, a continuation line whose container prefix is missing is
    // treated as a lazy paragraph continuation. The closing `}` (and
    // surrounding whitespace) may appear on the lazy line, but body
    // content there follows `allow_lazy_body` (set true for text
    // expressions, false for flow expressions — see
    // micromark-extension-mdx-expression's "Unexpected lazy line" rule).
    lazy_mode: bool,
    allow_lazy_body: bool,
    nesting_depth: u32,
) -> Option<usize> {
    if nesting_depth > MAX_MDX_NESTING {
        return None;
    }
    if bytes.is_empty() || bytes[0] != b'{' {
        return None;
    }

    let mut ix = 1;
    let mut depth: usize = 1;
    // Tracks "current line entered via lazy continuation"; only consulted
    // when `lazy_mode && !allow_lazy_body`, which is the flow-expression
    // case where body chars on a lazy line must abort the scan.
    let mut current_line_lazy = false;
    // Whether the previous semantically-relevant token produced a value
    // (identifier, literal, regex close, `)`, `]`, `}`); whitespace and
    // comments preserve it. Consumed by `slash_is_regex_after` to tell
    // division from a regex literal.
    let mut prev_was_value = false;
    macro_rules! mark_value {
        () => {
            prev_was_value = true;
        };
    }
    macro_rules! mark_op {
        () => {
            prev_was_value = false;
        };
    }

    macro_rules! reject_if_lazy {
        () => {
            if lazy_mode && !allow_lazy_body && current_line_lazy {
                return None;
            }
        };
    }

    while ix < bytes.len() && depth > 0 {
        match bytes[ix] {
            b'\n' => {
                if inline && is_blank_line_next(bytes, ix) {
                    return None;
                }
                ix += 1;
                if lazy_mode {
                    current_line_lazy =
                        check_container_after_newline_lazy(bytes, &mut ix, &container_check)
                            == LineMode::Lazy;
                } else {
                    check_container_after_newline(bytes, &mut ix, &container_check)?;
                }
            }
            b'\r' => {
                if inline && is_blank_line_next(bytes, ix) {
                    return None;
                }
                ix += 1;
                if ix < bytes.len() && bytes[ix] == b'\n' {
                    ix += 1;
                }
                if lazy_mode {
                    current_line_lazy =
                        check_container_after_newline_lazy(bytes, &mut ix, &container_check)
                            == LineMode::Lazy;
                } else {
                    check_container_after_newline(bytes, &mut ix, &container_check)?;
                }
            }
            b' ' | b'\t' => {
                ix += 1;
            }
            b'{' => {
                reject_if_lazy!();
                depth += 1;
                ix += 1;
                mark_op!();
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    // Flow-position expressions (strict-lazy) reject the
                    // closing brace too when it lands on a lazy line —
                    // matches micromark's `allowLazy: false` rule, which
                    // errors on *any* token while the line is lazy.
                    reject_if_lazy!();
                    return Some(ix + 1);
                }
                reject_if_lazy!();
                ix += 1;
                mark_value!();
            }
            // String literals (cannot span lines in JS). A `'` preceded by an
            // identifier char is almost certainly an apostrophe inside JSX
            // text (`user's`), not a string open — in that position it
            // couldn't be a valid JS expression anyway. Skip it as a regular
            // char so the apostrophe doesn't swallow the rest of the line.
            b'"' | b'\'' => {
                reject_if_lazy!();
                if bytes[ix] == b'\''
                    && ix > 0
                    && (bytes[ix - 1].is_ascii_alphanumeric() || bytes[ix - 1] == b'_')
                {
                    ix += 1;
                    continue;
                }
                ix = skip_string(bytes, ix);
                mark_value!();
            }
            // Template literals with ${} nesting.
            b'`' => {
                reject_if_lazy!();
                ix += 1;
                let mut template_depth: usize = 0;
                while ix < bytes.len() {
                    match bytes[ix] {
                        b'`' if template_depth == 0 => {
                            ix += 1;
                            break;
                        }
                        b'\\' => {
                            ix += 2;
                            continue;
                        }
                        b'$' if ix + 1 < bytes.len() && bytes[ix + 1] == b'{' => {
                            template_depth += 1;
                            ix += 2;
                            continue;
                        }
                        b'{' if template_depth > 0 => template_depth += 1,
                        b'}' if template_depth > 0 => template_depth -= 1,
                        b'\n' | b'\r' if inline && is_blank_line_next(bytes, ix) => {
                            return None;
                        }
                        _ => {}
                    }
                    ix += 1;
                }
                mark_value!();
            }
            b'/' if ix + 1 < bytes.len() && bytes[ix + 1] == b'/' => {
                reject_if_lazy!();
                ix += 2;
                while ix < bytes.len() && bytes[ix] != b'\n' {
                    ix += 1;
                }
            }
            b'/' if ix + 1 < bytes.len() && bytes[ix + 1] == b'*' => {
                reject_if_lazy!();
                ix += 2;
                while ix + 1 < bytes.len() {
                    if bytes[ix] == b'*' && bytes[ix + 1] == b'/' {
                        ix += 2;
                        break;
                    }
                    if inline
                        && (bytes[ix] == b'\n' || bytes[ix] == b'\r')
                        && is_blank_line_next(bytes, ix)
                    {
                        return None;
                    }
                    ix += 1;
                }
            }
            b'/' if slash_is_regex_after(bytes, ix, prev_was_value) => {
                reject_if_lazy!();
                ix = scan_regex(bytes, ix);
                mark_value!();
            }
            // JSX: skip a whole `<tag ...>...</tag>` element (or a self-closing
            // / closing tag). Scanning the element (not just the tag) consumes
            // its children as JSX text, where quotes and braces are literal and
            // can't be mis-lexed as a JS string that swallows the closing `}`.
            // When the lookahead doesn't form a valid JSX tag, treat the `<` as
            // a less-than operator: mark_op so a following `/` is read as a regex
            // (the acorn interpretation), not as division. Without this, an
            // expression like `l</:/}` mis-parses the trailing `/}` as a regex
            // literal and swallows the closing brace.
            b'<' if ix + 1 < bytes.len()
                && (bytes[ix + 1].is_ascii_alphabetic()
                    || bytes[ix + 1] == b'_'
                    || bytes[ix + 1] == b'$'
                    || bytes[ix + 1] == b'/'
                    || bytes[ix + 1] == b'>') =>
            {
                reject_if_lazy!();
                if let Some(end) = scan_mdx_jsx_element_end(&bytes[ix..], nesting_depth + 1) {
                    ix += end;
                    mark_value!();
                } else {
                    ix += 1;
                    mark_op!();
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' | b'0'..=b'9' => {
                reject_if_lazy!();
                ix += 1;
                mark_value!();
            }
            b')' | b']' => {
                reject_if_lazy!();
                ix += 1;
                mark_value!();
            }
            _ => {
                reject_if_lazy!();
                ix += 1;
                mark_op!();
            }
        }
    }
    None
}

/// Scan to the end of a line, returning offset past the newline.
fn scan_to_line_end(bytes: &[u8], start: usize) -> Option<usize> {
    let eol = memchr(b'\n', &bytes[start..])
        .map(|i| start + i + 1)
        .unwrap_or(bytes.len());
    Some(eol)
}

/// Scan a JSX tag from `<` to `>` or `/>`, returning the byte offset
/// immediately after the closing `>`. Does NOT scan to EOL.
fn scan_mdx_jsx_tag_end(bytes: &[u8]) -> Option<usize> {
    scan_mdx_jsx_tag_end_inner(bytes, None, 0)
}

fn scan_mdx_jsx_tag_end_inner(
    bytes: &[u8],
    container_check: Option<ContainerLineCheck<'_>>,
    nesting_depth: u32,
) -> Option<usize> {
    if nesting_depth > MAX_MDX_NESTING {
        return None;
    }
    let mut ix = 1; // skip `<`

    // Skip `/` for closing tags
    let is_closing = ix < bytes.len() && bytes[ix] == b'/';
    if is_closing {
        ix += 1;
        // micromark-extension-mdx-jsx routes through `esWhitespaceStart`
        // after the `/`, which accepts both markdown whitespace (` `, `\t`,
        // line endings) and Unicode ES whitespace before the closing-tag
        // name or `>`.
        while ix < bytes.len() && is_mdx_unicode_whitespace(bytes, ix) {
            ix += char_len_utf8(bytes[ix]);
        }
    }

    // Fragment `<>` / `</>`
    if ix < bytes.len() && bytes[ix] == b'>' {
        return Some(ix + 1);
    }

    // Validate tag name: must start with a valid identifier start character
    if ix >= bytes.len() || !is_jsx_name_start(bytes, ix) {
        return None;
    }
    ix += char_len_utf8(bytes[ix]);

    // Scan tag name body. JSX names are either a plain name, a namespaced
    // name (`a:b` — exactly one `:`, no member chain after), or a member
    // chain (`a.b.c` — any number of `.` segments, each a fresh name-start).
    // Mixing namespace and member (`a:b.c`) is rejected by mdx-js, and so
    // is a name-continue char following `.` (e.g. `a..b`, `a.1`).
    //
    // micromark permits whitespace around `.` and `:` separators
    // (`<a :b/>` → name `a:b`, `<a . b/>` → name `a.b`), so we
    // peek past intra-name whitespace before deciding the loop is done.
    let mut saw_namespace = false;
    let mut saw_member = false;
    loop {
        while ix < bytes.len() && is_jsx_name_continue(bytes, ix) {
            ix += char_len_utf8(bytes[ix]);
        }
        // If the (optional ws) + (`:` or `.`) + (optional ws) + name-start
        // peek fails, restore `ix` — the skipped bytes become the post-name
        // area for the caller's structural checks (ws / `>` / `/` / `{` /
        // attribute start).
        let save = ix;
        while ix < bytes.len() && is_mdx_unicode_whitespace(bytes, ix) {
            ix += char_len_utf8(bytes[ix]);
        }
        if ix >= bytes.len() {
            ix = save;
            break;
        }
        match bytes[ix] {
            b':' => {
                // mdx-js rejects namespace after a member (`a.b:c`) and a
                // second namespace marker (`a:b:c`).
                if saw_namespace || saw_member {
                    ix = save;
                    break;
                }
                saw_namespace = true;
                ix += 1;
            }
            b'.' => {
                if saw_namespace {
                    ix = save;
                    break;
                }
                saw_member = true;
                ix += 1;
            }
            _ => {
                ix = save;
                break;
            }
        }
        // After `:` or `.`, optional whitespace then a name-start.
        while ix < bytes.len() && is_mdx_unicode_whitespace(bytes, ix) {
            ix += char_len_utf8(bytes[ix]);
        }
        if ix >= bytes.len() || !is_jsx_name_start(bytes, ix) {
            return None;
        }
        ix += char_len_utf8(bytes[ix]);
    }

    // After tag name: must be whitespace, `>`, `/`, or `{`
    if ix < bytes.len() {
        match bytes[ix] {
            b'>' | b'/' | b'{' => {}
            _ if is_mdx_unicode_whitespace(bytes, ix) => {}
            _ => return None,
        }
    }

    // Attribute area. Closing tags accept only whitespace before `>`;
    // opening tags accept attribute names, attribute values, and spread
    // expressions. Validate each attribute structurally — mdx-js rejects
    // garbage like `<a 1x/>` or `<a x=foo/>`.
    loop {
        // Skip inter-attribute whitespace (incl. newlines).
        while ix < bytes.len() {
            match bytes[ix] {
                b' ' | b'\t' => ix += 1,
                b'\n' | b'\r' => {
                    let was_cr = bytes[ix] == b'\r';
                    ix += 1;
                    if was_cr && ix < bytes.len() && bytes[ix] == b'\n' {
                        ix += 1;
                    }
                    check_container_after_newline(bytes, &mut ix, &container_check)?;
                }
                _ if is_mdx_unicode_whitespace(bytes, ix) => {
                    ix += char_len_utf8(bytes[ix]);
                }
                _ => break,
            }
        }
        if ix >= bytes.len() {
            return None;
        }
        match bytes[ix] {
            b'>' => return Some(ix + 1),
            // Closing tags allow only whitespace before `>` — no attrs, no
            // self-close marker. Reject before the `/` arm so `</a />` and
            // `</a/ >` aren't treated as self-closing.
            _ if is_closing => return None,
            // Self-close marker: `/` followed (possibly across whitespace
            // or a newline+container-prefix) by `>`. mdx-js accepts e.g.
            // `<g/\n>` and `<a / >`.
            b'/' => {
                let mut j = ix + 1;
                while j < bytes.len() {
                    match bytes[j] {
                        b' ' | b'\t' => j += 1,
                        b'\n' | b'\r' => {
                            let was_cr = bytes[j] == b'\r';
                            j += 1;
                            if was_cr && j < bytes.len() && bytes[j] == b'\n' {
                                j += 1;
                            }
                            check_container_after_newline(bytes, &mut j, &container_check)?;
                        }
                        _ if is_mdx_unicode_whitespace(bytes, j) => {
                            j += char_len_utf8(bytes[j]);
                        }
                        _ => break,
                    }
                }
                if j < bytes.len() && bytes[j] == b'>' {
                    return Some(j + 1);
                }
                return None;
            }
            // Spread or shorthand expression — must be `{...expr}` for a
            // spread; bare `{expr}` is rejected by mdx-js.
            b'{' => {
                if !looks_like_spread(&bytes[ix..]) {
                    return None;
                }
                let expr_len = scan_mdx_expression_end_inner(
                    &bytes[ix..],
                    false,
                    None,
                    false,
                    true,
                    nesting_depth + 1,
                )?;
                ix += expr_len;
            }
            // Attribute name + optional value.
            _ if is_jsx_name_start(bytes, ix) => {
                ix += char_len_utf8(bytes[ix]);
                let mut attr_saw_namespace = false;
                while ix < bytes.len() {
                    if bytes[ix] == b':' {
                        if attr_saw_namespace {
                            return None;
                        }
                        attr_saw_namespace = true;
                        ix += 1;
                        if ix >= bytes.len() || !is_jsx_name_start(bytes, ix) {
                            return None;
                        }
                        ix += char_len_utf8(bytes[ix]);
                    } else if is_jsx_name_continue(bytes, ix) {
                        ix += char_len_utf8(bytes[ix]);
                    } else {
                        break;
                    }
                }
                let mut peek = ix;
                while peek < bytes.len() {
                    match bytes[peek] {
                        b' ' | b'\t' => peek += 1,
                        b'\n' | b'\r' => {
                            let was_cr = bytes[peek] == b'\r';
                            peek += 1;
                            if was_cr && peek < bytes.len() && bytes[peek] == b'\n' {
                                peek += 1;
                            }
                            check_container_after_newline(bytes, &mut peek, &container_check)?;
                        }
                        _ if is_mdx_unicode_whitespace(bytes, peek) => {
                            peek += char_len_utf8(bytes[peek]);
                        }
                        _ => break,
                    }
                }
                if peek < bytes.len() && bytes[peek] == b'=' {
                    ix = peek + 1;
                    while ix < bytes.len() {
                        match bytes[ix] {
                            b' ' | b'\t' => ix += 1,
                            b'\n' | b'\r' => {
                                let was_cr = bytes[ix] == b'\r';
                                ix += 1;
                                if was_cr && ix < bytes.len() && bytes[ix] == b'\n' {
                                    ix += 1;
                                }
                                check_container_after_newline(bytes, &mut ix, &container_check)?;
                            }
                            _ if is_mdx_unicode_whitespace(bytes, ix) => {
                                ix += char_len_utf8(bytes[ix]);
                            }
                            _ => break,
                        }
                    }
                    if ix >= bytes.len() {
                        return None;
                    }
                    match bytes[ix] {
                        // JSX attribute strings are HTML-like: no backslash
                        // escapes, so `\"` is two literal chars and the
                        // string still terminates at the next `"`.
                        b'"' => {
                            ix += 1;
                            while ix < bytes.len() && bytes[ix] != b'"' {
                                ix += 1;
                            }
                            if ix >= bytes.len() {
                                return None;
                            }
                            ix += 1;
                        }
                        b'\'' => {
                            ix += 1;
                            while ix < bytes.len() && bytes[ix] != b'\'' {
                                ix += 1;
                            }
                            if ix >= bytes.len() {
                                return None;
                            }
                            ix += 1;
                        }
                        b'{' => {
                            let expr_len = scan_mdx_expression_end_inner(
                                &bytes[ix..],
                                false,
                                None,
                                false,
                                true,
                                nesting_depth + 1,
                            )?;
                            ix += expr_len;
                        }
                        // Bare-word attribute values (`<a x=foo/>`) are
                        // rejected by mdx-js.
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
    }
}

/// Cheap lookahead: does `bytes` (starting at a `{`) look like a spread
/// expression `{...expr}`? Used to reject `<a {x}/>` (bare expression in
/// attribute position) at scan time. Whitespace between `{` and `...` is
/// tolerated to mirror `acorn`'s leniency.
fn looks_like_spread(bytes: &[u8]) -> bool {
    debug_assert_eq!(bytes.first(), Some(&b'{'));
    let mut i = 1;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i + 2 < bytes.len() && &bytes[i..i + 3] == b"..."
}

/// Scan a full JSX element beginning at `bytes[0] == '<'`, returning the byte
/// offset past its matching close tag (or past `/>` for a self-closing element,
/// or past the tag for a bare closing tag). Returns `None` if the bytes don't
/// open a valid JSX tag.
///
/// Unlike [`scan_mdx_jsx_tag_end_inner`] (which scans a single tag), this also
/// descends through the element's children: text is consumed literally (so
/// quotes and braces in JSX text are NOT lexed as JS) while nested `{...}` are
/// scanned as expressions and nested `<...>` as elements. That is what lets a
/// quote in JSX text inside an attribute expression (`d={<p>Acme Corp.'s "best"
/// tool</p>}`, `d={<p>a<b>x</b>'s</p>}`) avoid being mistaken for a string
/// literal that swallows the closing brace.
fn scan_mdx_jsx_element_end(bytes: &[u8], nesting_depth: u32) -> Option<usize> {
    if nesting_depth > MAX_MDX_NESTING {
        return None;
    }
    // The opening (or closing, or fragment) tag itself.
    let tag_end = scan_mdx_jsx_tag_end_inner(bytes, None, nesting_depth)?;
    // A closing tag (`</x>`) or self-closing tag (`<x/>`) has no children.
    let is_closing = bytes.len() > 1 && bytes[1] == b'/';
    let is_self_closing = tag_end >= 2 && bytes[tag_end - 2] == b'/';
    if is_closing || is_self_closing {
        return Some(tag_end);
    }
    // Opening tag (or fragment `<>`): scan children to the matching close tag.
    let mut ix = tag_end;
    while ix < bytes.len() {
        match bytes[ix] {
            // Nested expression child; JS lexing resumes inside it.
            b'{' => {
                let len = scan_mdx_expression_end_inner(
                    &bytes[ix..],
                    false,
                    None,
                    false,
                    true,
                    nesting_depth,
                )?;
                ix += len;
            }
            // The close tag for this element.
            b'<' if ix + 1 < bytes.len() && bytes[ix + 1] == b'/' => {
                let len = scan_mdx_jsx_tag_end_inner(&bytes[ix..], None, nesting_depth)?;
                return Some(ix + len);
            }
            // A nested element, or a literal `<` in text (`a < b`). Try to scan
            // an element; if it isn't one, the `<` is just text.
            b'<' => {
                if let Some(len) = scan_mdx_jsx_element_end(&bytes[ix..], nesting_depth + 1) {
                    ix += len;
                } else {
                    ix += 1;
                }
            }
            // Any other byte (quotes and apostrophes included) is literal text.
            _ => ix += 1,
        }
    }
    None
}

/// Scan for an MDX ESM block (`import ...` or `export ...`).
///
/// Greedily consumes continuation lines until a blank line ends the block,
/// matching the reference mdxjs behavior. The completeness check for blocks
/// that span a blank line in plain code (e.g. a multi-line object literal) is
/// done later by the firstpass using oxc.
///
/// Template literals and block comments can legitimately contain a blank line,
/// so the scan tracks just enough JS lexer state to not end the block inside
/// one — otherwise `export const x = ` `` `a\n\nb` `` truncates mid-template and
/// oxc reports an unterminated string. Strings and line comments can't cross a
/// newline in valid JS, so they need no cross-line state, but regex literals
/// are still skipped whole: a backtick or quote inside one (`/[`]/`) would
/// otherwise be mistaken for a template/string opener and swallow the block
/// past the blank line that should end it.
///
/// Returns the byte offset past the end of the block (including trailing newline).
pub(crate) fn scan_mdx_esm(bytes: &[u8]) -> Option<usize> {
    let is_import = bytes.starts_with(b"import ")
        || bytes.starts_with(b"import\t")
        || bytes.starts_with(b"import{");
    let is_export = bytes.starts_with(b"export ")
        || bytes.starts_with(b"export\t")
        || bytes.starts_with(b"export{")
        || bytes.starts_with(b"export*")
        || bytes.starts_with(b"export\n")
        || bytes.starts_with(b"export\r");

    if !is_import && !is_export {
        return None;
    }

    let len = bytes.len();
    let mut ix = 0;
    // `Some(depth)` while inside a template literal, where `depth` counts open
    // `${` interpolation braces (same model as `scan_mdx_expression_end_inner`,
    // including its limitation around nested templates / strings in `${}`).
    let mut template_depth: Option<usize> = None;
    let mut in_block_comment = false;
    // Whether the previous token produced a value; whitespace, newlines, and
    // comments preserve it. Consumed by `slash_is_regex_after`.
    let mut prev_was_value = false;

    while ix < len {
        if in_block_comment {
            if bytes[ix] == b'*' && ix + 1 < len && bytes[ix + 1] == b'/' {
                in_block_comment = false;
                ix += 2;
            } else {
                ix += 1;
            }
            continue;
        }
        if let Some(depth) = template_depth {
            match bytes[ix] {
                b'\\' => ix += 2,
                b'`' if depth == 0 => {
                    template_depth = None;
                    prev_was_value = true;
                    ix += 1;
                }
                b'$' if ix + 1 < len && bytes[ix + 1] == b'{' => {
                    template_depth = Some(depth + 1);
                    ix += 2;
                }
                b'{' if depth > 0 => {
                    template_depth = Some(depth + 1);
                    ix += 1;
                }
                b'}' if depth > 0 => {
                    template_depth = Some(depth - 1);
                    ix += 1;
                }
                _ => ix += 1,
            }
            continue;
        }
        match bytes[ix] {
            b'`' => {
                // `prev_was_value` is set when the template closes.
                template_depth = Some(0);
                ix += 1;
            }
            b'\'' | b'"' => {
                ix = skip_string(bytes, ix);
                prev_was_value = true;
            }
            b'/' if ix + 1 < len && bytes[ix + 1] == b'/' => {
                while ix < len && bytes[ix] != b'\n' {
                    ix += 1;
                }
            }
            b'/' if ix + 1 < len && bytes[ix + 1] == b'*' => {
                in_block_comment = true;
                ix += 2;
            }
            // A real regex must be consumed whole so a backtick or quote inside
            // it isn't read as a template/string opener. `scan_regex` stops at a
            // newline, so even a misread can't cross the blank line that ends
            // the block.
            b'/' if slash_is_regex_after(bytes, ix, prev_was_value) => {
                ix = scan_regex(bytes, ix);
                prev_was_value = true;
            }
            b'\n' => {
                ix += 1;
                if ix >= len {
                    break;
                }
                // A line of only spaces/tabs is blank, exactly like an empty
                // line, and ends the block. The firstpass retries across it
                // with oxc when the block is still incomplete (e.g. an export
                // object spanning a blank line in plain code).
                let mut next = ix;
                while next < len && matches!(bytes[next], b' ' | b'\t') {
                    next += 1;
                }
                if next >= len || matches!(bytes[next], b'\n' | b'\r') {
                    break;
                }
            }
            b' ' | b'\t' | b'\r' => ix += 1,
            b')' | b']' | b'}' => {
                prev_was_value = true;
                ix += 1;
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' | b'0'..=b'9' => {
                prev_was_value = true;
                ix += 1;
            }
            _ => {
                prev_was_value = false;
                ix += 1;
            }
        }
    }
    Some(ix)
}

/// Check if `<` starts a **block-level** MDX JSX element.
/// A block JSX element must be the only significant content on the line
/// (possibly followed only by whitespace). If there's trailing text, it's
/// inline JSX inside a paragraph instead.
///
/// Returns byte offset past the element (including trailing newline) if matched.
pub(crate) fn scan_mdx_jsx_block(
    bytes: &[u8],
    container_check: Option<ContainerLineCheck<'_>>,
) -> Option<usize> {
    if bytes.len() < 2 || bytes[0] != b'<' {
        return None;
    }

    let is_closing = bytes[1] == b'/';
    let mut name_start = if is_closing { 2 } else { 1 };

    // Micromark-extension-mdx-jsx allows ES whitespace (including line
    // endings) between `</` and the closing tag name / `>` — e.g.
    // `</ Name>` or `</\n  Name>`. Probe past it for the name-start
    // check, but leave `scan_mdx_jsx_tag_end_inner` to consume the same
    // whitespace so positions stay aligned.
    if is_closing {
        while name_start < bytes.len() && is_mdx_unicode_whitespace(bytes, name_start) {
            name_start += char_len_utf8(bytes[name_start]);
        }
    }

    if name_start >= bytes.len() {
        return None;
    }

    // Fragment: `<>` or `</>`
    let mut pos = if bytes[name_start] == b'>' {
        name_start + 1
    } else {
        if !is_jsx_name_start(bytes, name_start) {
            return None;
        }
        scan_mdx_jsx_tag_end_inner(bytes, container_check, 0)?
    };

    // Consume any subsequent JSX tags or expressions on the same line.
    // Bare text rejects flow — the line falls through to paragraph parsing
    // where the JSX becomes inline MdxJsxTextElement nodes.
    // Two consecutive expressions without a JSX tag between them also reject flow,
    // matching micromark-extension-mdx-jsx behavior.
    let mut last_was_jsx = true;
    loop {
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] == b'\n' || bytes[pos] == b'\r' {
            break;
        }
        if bytes[pos] == b'<' {
            if let Some(end) = scan_mdx_jsx_tag_end_inner(&bytes[pos..], container_check, 0) {
                pos += end;
                last_was_jsx = true;
                continue;
            }
        }
        if bytes[pos] == b'{' {
            // Flow context: child expressions can span multiple lines —
            // including blank lines inside template literals (e.g.
            // `<style>{`...CSS with blank line...`}</style>`). Pass
            // `inline=false` so the expression scanner doesn't bail at the
            // first blank line.
            if let Some(len) = scan_mdx_expression_end(&bytes[pos..], false) {
                if !last_was_jsx {
                    return None;
                }
                pos += len;
                last_was_jsx = false;
                continue;
            }
        }
        return None;
    }

    scan_to_line_end(bytes, pos)
}

/// Scan for an MDX expression block: `{...}` using lexer-based boundary detection.
/// Returns byte offset past the end (including trailing newline).
pub(crate) fn scan_mdx_expression_block(
    bytes: &[u8],
    container_check: Option<ContainerLineCheck<'_>>,
) -> Option<usize> {
    let mut ix = scan_mdx_expression_end_inner(bytes, false, container_check, false, true, 0)?;
    let mut last_was_jsx = false;

    loop {
        while ix < bytes.len() && (bytes[ix] == b' ' || bytes[ix] == b'\t') {
            ix += 1;
        }
        if ix >= bytes.len() || bytes[ix] == b'\n' || bytes[ix] == b'\r' {
            break;
        }
        if bytes[ix] == b'<' {
            if let Some(end) = scan_mdx_jsx_tag_end_inner(&bytes[ix..], container_check, 0) {
                ix += end;
                last_was_jsx = true;
                continue;
            }
            if ix + 1 < bytes.len() && bytes[ix + 1] == b'>' {
                ix += 2;
                last_was_jsx = true;
                continue;
            }
            if ix + 2 < bytes.len() && bytes[ix + 1] == b'/' && bytes[ix + 2] == b'>' {
                ix += 3;
                last_was_jsx = true;
                continue;
            }
        }
        if bytes[ix] == b'{' {
            if let Some(len) = scan_mdx_expression_end(&bytes[ix..], true) {
                if !last_was_jsx {
                    return None;
                }
                ix += len;
                last_was_jsx = false;
                continue;
            }
        }
        return None;
    }

    scan_to_line_end(bytes, ix)
}

/// Scan an inline MDX expression: `{...}` using lexer-based boundary detection.
/// Returns (content_start, content_end, total_len) where content excludes the outer braces.
pub(crate) fn scan_mdx_inline_expression(bytes: &[u8]) -> Option<(usize, usize, usize)> {
    let total = scan_mdx_expression_end(bytes, true)?;
    Some((1, total - 1, total))
}

pub(crate) fn scan_mdx_inline_expression_in_container(
    bytes: &[u8],
    container_check: ContainerLineCheck<'_>,
    allow_lazy_body: bool,
) -> Option<(usize, usize, usize)> {
    // `allow_lazy_body`: text-position expressions (preceded by content on
    // the line) follow micromark's `allowLazy: true` text tokenizer and
    // accept body content on lazy continuation lines. Flow-position `{`
    // (first content of a paragraph line in a container) follows the
    // `allowLazy: false` flow tokenizer; the lazy line is consumed but
    // body content there fails the scan.
    let total = scan_mdx_expression_end_inner(
        bytes,
        true,
        Some(container_check),
        true,
        allow_lazy_body,
        0,
    )?;
    Some((1, total - 1, total))
}

/// Scan an inline JSX tag from `<` to `>` or `/>`.
/// In MDX mode, ALL tags are JSX. Returns total byte length if matched.
pub(crate) fn scan_mdx_inline_jsx(bytes: &[u8]) -> Option<usize> {
    scan_mdx_inline_jsx_inner(bytes, None)
}

fn scan_mdx_inline_jsx_inner(
    bytes: &[u8],
    container_check: Option<ContainerLineCheck<'_>>,
) -> Option<usize> {
    if bytes.len() < 2 || bytes[0] != b'<' {
        return None;
    }

    let is_closing = bytes[1] == b'/';
    let mut name_start = if is_closing { 2 } else { 1 };

    // Micromark-extension-mdx-jsx tolerates ES whitespace (incl. line
    // endings) between `</` and the closing tag name / `>` — e.g.
    // `</ Name>` or `</\n>` (fragment close split over lines).
    if is_closing {
        while name_start < bytes.len() && is_mdx_unicode_whitespace(bytes, name_start) {
            name_start += char_len_utf8(bytes[name_start]);
        }
    }

    if name_start >= bytes.len() {
        return None;
    }

    // Fragment: `<>` or `</>`
    if bytes[name_start] == b'>' {
        return Some(name_start + 1);
    }

    if !is_jsx_name_start(bytes, name_start) {
        return None;
    }

    let mut ix = name_start + char_len_utf8(bytes[name_start]);
    let mut saw_namespace = false;
    let mut saw_member = false;
    loop {
        while ix < bytes.len() && is_jsx_name_continue(bytes, ix) {
            ix += char_len_utf8(bytes[ix]);
        }
        let save = ix;
        while ix < bytes.len() && is_mdx_unicode_whitespace(bytes, ix) {
            ix += char_len_utf8(bytes[ix]);
        }
        if ix >= bytes.len() {
            ix = save;
            break;
        }
        match bytes[ix] {
            b':' => {
                if saw_namespace || saw_member {
                    ix = save;
                    break;
                }
                saw_namespace = true;
                ix += 1;
            }
            b'.' => {
                if saw_namespace {
                    ix = save;
                    break;
                }
                saw_member = true;
                ix += 1;
            }
            _ => {
                ix = save;
                break;
            }
        }
        while ix < bytes.len() && is_mdx_unicode_whitespace(bytes, ix) {
            ix += char_len_utf8(bytes[ix]);
        }
        if ix >= bytes.len() || !is_jsx_name_start(bytes, ix) {
            return None;
        }
        ix += char_len_utf8(bytes[ix]);
    }

    // After the tag name: must be whitespace, `>`, `/`, or `{`.
    // This rejects patterns like `<https://...>`.
    if ix < bytes.len() {
        match bytes[ix] {
            b'>' | b'/' | b'{' => {}
            _ if is_mdx_unicode_whitespace(bytes, ix) => {}
            _ => return None,
        }
    }

    // Attribute area. Mirrors `scan_mdx_jsx_tag_end_inner`: each iteration
    // skips whitespace, then expects `>` / `/>`, a spread `{...expr}`, or a
    // valid attribute name (with optional `="value"` / `={expr}`). Closing
    // tags accept only whitespace before `>`.
    loop {
        while ix < bytes.len() {
            match bytes[ix] {
                b' ' | b'\t' => ix += 1,
                b'\n' | b'\r' => {
                    let was_cr = bytes[ix] == b'\r';
                    ix += 1;
                    if was_cr && ix < bytes.len() && bytes[ix] == b'\n' {
                        ix += 1;
                    }
                    check_container_after_newline(bytes, &mut ix, &container_check)?;
                }
                _ if is_mdx_unicode_whitespace(bytes, ix) => {
                    ix += char_len_utf8(bytes[ix]);
                }
                _ => break,
            }
        }
        if ix >= bytes.len() {
            return None;
        }
        match bytes[ix] {
            b'>' => return Some(ix + 1),
            // Closing tags accept only whitespace before `>` — reject before
            // the `/` arm so `</a />` doesn't slip through as self-closing.
            _ if is_closing => return None,
            b'/' => {
                let mut j = ix + 1;
                while j < bytes.len() {
                    match bytes[j] {
                        b' ' | b'\t' => j += 1,
                        b'\n' | b'\r' => {
                            let was_cr = bytes[j] == b'\r';
                            j += 1;
                            if was_cr && j < bytes.len() && bytes[j] == b'\n' {
                                j += 1;
                            }
                            check_container_after_newline(bytes, &mut j, &container_check)?;
                        }
                        _ if is_mdx_unicode_whitespace(bytes, j) => {
                            j += char_len_utf8(bytes[j]);
                        }
                        _ => break,
                    }
                }
                if j < bytes.len() && bytes[j] == b'>' {
                    return Some(j + 1);
                }
                return None;
            }
            b'{' => {
                if !looks_like_spread(&bytes[ix..]) {
                    return None;
                }
                let expr_len = scan_mdx_expression_end(&bytes[ix..], false)?;
                ix += expr_len;
            }
            _ if is_jsx_name_start(bytes, ix) => {
                ix += char_len_utf8(bytes[ix]);
                let mut attr_saw_namespace = false;
                while ix < bytes.len() {
                    if bytes[ix] == b':' {
                        if attr_saw_namespace {
                            return None;
                        }
                        attr_saw_namespace = true;
                        ix += 1;
                        if ix >= bytes.len() || !is_jsx_name_start(bytes, ix) {
                            return None;
                        }
                        ix += char_len_utf8(bytes[ix]);
                    } else if is_jsx_name_continue(bytes, ix) {
                        ix += char_len_utf8(bytes[ix]);
                    } else {
                        break;
                    }
                }
                // mdx-js accepts whitespace on either side of `=`, e.g.
                // `<Foo bar = "x"/>` or `<Foo bar =\n  {1}/>`. Peek past
                // whitespace to find an `=` that belongs to this attribute.
                let mut peek = ix;
                while peek < bytes.len() {
                    match bytes[peek] {
                        b' ' | b'\t' => peek += 1,
                        b'\n' | b'\r' => {
                            let was_cr = bytes[peek] == b'\r';
                            peek += 1;
                            if was_cr && peek < bytes.len() && bytes[peek] == b'\n' {
                                peek += 1;
                            }
                            check_container_after_newline(bytes, &mut peek, &container_check)?;
                        }
                        _ if is_mdx_unicode_whitespace(bytes, peek) => {
                            peek += char_len_utf8(bytes[peek]);
                        }
                        _ => break,
                    }
                }
                if peek < bytes.len() && bytes[peek] == b'=' {
                    ix = peek + 1;
                    while ix < bytes.len() {
                        match bytes[ix] {
                            b' ' | b'\t' => ix += 1,
                            b'\n' | b'\r' => {
                                let was_cr = bytes[ix] == b'\r';
                                ix += 1;
                                if was_cr && ix < bytes.len() && bytes[ix] == b'\n' {
                                    ix += 1;
                                }
                                check_container_after_newline(bytes, &mut ix, &container_check)?;
                            }
                            _ if is_mdx_unicode_whitespace(bytes, ix) => {
                                ix += char_len_utf8(bytes[ix]);
                            }
                            _ => break,
                        }
                    }
                    if ix >= bytes.len() {
                        return None;
                    }
                    match bytes[ix] {
                        // JSX attribute strings are HTML-like: no backslash
                        // escapes, so `\"` is two literal chars.
                        b'"' => {
                            ix += 1;
                            while ix < bytes.len() && bytes[ix] != b'"' {
                                ix += 1;
                            }
                            if ix >= bytes.len() {
                                return None;
                            }
                            ix += 1;
                        }
                        b'\'' => {
                            ix += 1;
                            while ix < bytes.len() && bytes[ix] != b'\'' {
                                ix += 1;
                            }
                            if ix >= bytes.len() {
                                return None;
                            }
                            ix += 1;
                        }
                        b'{' => {
                            let expr_len = scan_mdx_expression_end(&bytes[ix..], false)?;
                            ix += expr_len;
                        }
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
    }
}

impl<'a, 'b> FirstPass<'a, 'b> {
    pub(crate) fn parse_mdx_esm(&mut self, start_ix: usize, end_ix: usize) -> usize {
        // Strip only trailing line terminators — remark keeps trailing spaces
        // (e.g. `"import X; "`) in the mdxjsEsm value.
        let content = self.text[start_ix..end_ix].trim_end_matches(['\n', '\r']);
        let cow_ix = self.allocs.allocate_cow(content.into());
        self.tree.append(Item {
            start: start_ix,
            end: end_ix,
            body: ItemBody::MdxEsm(cow_ix),
        });
        end_ix
    }

    pub(crate) fn parse_mdx_jsx_flow(&mut self, start_ix: usize, end_ix: usize) -> usize {
        let raw = {
            let stripped = self.strip_container_prefixes(start_ix, end_ix);
            stripped.trim_end().to_string()
        };

        // Map stripped-string offsets back to original-source byte offsets.
        // `strip_container_prefixes` removes container-continuation prefixes
        // from every line after the first and may prepend phantom spaces when
        // the prefix partially consumed a tab. Build per-line breakpoints so
        // we can convert any stripped offset back to source.
        //
        // Each entry: (stripped_line_start, orig_line_start, phantom_count)
        //   * positions in [stripped_line_start, stripped_line_start + phantom_count)
        //     map to `orig_line_start` (phantom region has no source bytes).
        //   * positions beyond that map 1:1: orig = orig_line_start + (s - stripped_line_start - phantom_count).
        let orig_bytes = self.text.as_bytes();
        let stripped_bytes = raw.as_bytes();
        let mut map: Vec<(usize, usize, usize)> = Vec::new();
        map.push((0, start_ix, 0));
        {
            let mut s_pos = 0usize;
            let mut o_pos = start_ix;
            while s_pos < stripped_bytes.len() && o_pos < end_ix {
                let b = stripped_bytes[s_pos];
                if b == b'\n' || b == b'\r' {
                    s_pos += 1;
                    o_pos += 1;
                    if b == b'\r'
                        && s_pos < stripped_bytes.len()
                        && stripped_bytes[s_pos] == b'\n'
                        && o_pos < end_ix
                        && orig_bytes[o_pos] == b'\n'
                    {
                        s_pos += 1;
                        o_pos += 1;
                    }
                    // After the newline we're at the start of a continuation
                    // line — skip the container prefix in the original source.
                    let mut ls = crate::scanners::LineStart::new(&orig_bytes[o_pos..end_ix]);
                    let _ = crate::parse::scan_containers(&self.tree, &mut ls, self.options);
                    o_pos += ls.bytes_scanned();
                    let phantom = ls.remaining_space();
                    map.push((s_pos, o_pos, phantom));
                    // Skip over phantom bytes in the stripped buffer.
                    s_pos += phantom;
                } else {
                    s_pos += 1;
                    o_pos += 1;
                }
            }
        }
        let stripped_to_orig = |s_pos: usize| -> usize {
            let idx = match map.binary_search_by(|probe| probe.0.cmp(&s_pos)) {
                Ok(i) => i,
                Err(i) => i.saturating_sub(1),
            };
            let (base_s, base_o, phantom) = map[idx];
            let offset = s_pos - base_s;
            if offset <= phantom {
                base_o
            } else {
                base_o + (offset - phantom)
            }
        };

        let mut pos = 0;
        while pos < raw.len() {
            while pos < raw.len() && raw.as_bytes()[pos] == b' ' {
                pos += 1;
            }
            if pos >= raw.len() {
                break;
            }
            let remaining = &raw.as_bytes()[pos..];
            if remaining[0] == b'<' {
                // On scan failure, emit a recovery node spanning the rest
                // of the block rather than erroring — the dispatcher
                // already committed to JSX flow.
                let tag_end = scan_mdx_jsx_tag_end(remaining).unwrap_or(raw.len() - pos);
                let tag_raw = &raw[pos..pos + tag_end];
                let container_content_col = self.container_content_col();
                let extra_strip_cols = self.directive_initial_size_sum();
                let jsx_data =
                    parse_jsx_tag_with_column(tag_raw, container_content_col, extra_strip_cols)
                        .into_static();
                validate_jsx_expressions(
                    tag_raw,
                    &jsx_data.attrs,
                    |rel| stripped_to_orig(pos + rel),
                    &mut self.mdx_expr_allocator,
                    &mut self.mdx_errors,
                );
                let jsx_ix = self.allocs.allocate_jsx_element(jsx_data);
                self.tree.append(Item {
                    start: stripped_to_orig(pos),
                    end: stripped_to_orig(pos + tag_end),
                    body: ItemBody::MdxJsxFlowElement(jsx_ix),
                });
                pos += tag_end;
            } else if remaining[0] == b'{' {
                // Flow context: expression body can legitimately span blank
                // lines (e.g. `<style>{\` CSS with blank rules \`}</style>`).
                let expr_end = scan_mdx_expression_end(remaining, false).unwrap_or(raw.len() - pos);
                let inner_raw = &raw[pos + 1..pos + expr_end - 1];
                // Validate the expression body as JS. mdx-js calls acorn here;
                // we use oxc for parity. Without this, garbage like `{h<}`
                // produces a phantom `mdxFlowExpression` that only errors at JS
                // emit, not at mdast. Validate the verbatim slice (not the
                // dedented `inner`) so the error offset stays in `raw`
                // coordinates and `stripped_to_orig` maps it to the exact
                // source position. Allocator is reused.
                if let Some((err_offset, detail)) =
                    validate_expression_body(inner_raw, &mut self.mdx_expr_allocator)
                {
                    self.mdx_errors.push((
                        stripped_to_orig(pos + 1 + err_offset),
                        alloc::format!("Could not parse expression with oxc: {detail}"),
                    ));
                }
                let inner: CowStr<'static> = CowStr::from(
                    dedent_expression_continuation(inner_raw, self.container_content_col())
                        .into_owned(),
                );
                let cow_ix = self.allocs.allocate_cow(inner);
                self.tree.append(Item {
                    start: stripped_to_orig(pos),
                    end: stripped_to_orig(pos + expr_end),
                    body: ItemBody::MdxFlowExpression(cow_ix),
                });
                pos += expr_end;
            } else {
                break;
            }
        }
        end_ix
    }

    /// Sum of `initialSize` across all enclosing ContainerDirective ancestors.
    /// Each contributes that many extra cols to the dedent applied to a
    /// multi-line MDX expression value inside the directive body — mirroring
    /// `micromark-extension-directive`'s `factorySpace(initialSize + 1)` strip.
    pub(crate) fn directive_initial_size_sum(&self) -> usize {
        use crate::parse::ItemBody;
        if !self.options.contains(crate::Options::ENABLE_DIRECTIVE) {
            return 0;
        }
        let mut sum = 0usize;
        for &node_ix in self.tree.walk_spine() {
            if let ItemBody::ContainerDirective(_, dir_ix) = self.tree[node_ix].item.body {
                sum += self.allocs.directive_ref(dir_ix).initial_size as usize;
            }
        }
        sum
    }

    /// Compute the column (1-indexed) at which the innermost list/blockquote
    /// container's content begins. Used for attribute-expression continuation
    /// line indent stripping.
    pub(crate) fn container_content_col(&self) -> usize {
        use crate::parse::ItemBody;
        let mut col = 1usize;
        for &node_ix in self.tree.walk_spine() {
            match self.tree[node_ix].item.body {
                ItemBody::BlockQuote(..) => col += 2, // `>` + space
                ItemBody::ListItem(indent, _) | ItemBody::DefinitionListDefinition(indent) => {
                    col += indent
                }
                ItemBody::FootnoteDefinition(..)
                    if self.options.contains(crate::Options::ENABLE_FOOTNOTES) =>
                {
                    col += 4
                }
                // Container directives don't add a per-line prefix strip
                // (strip_container_prefixes leaves directive body lines
                // untouched), so they don't shift `container_content_col`.
                // Their `initialSize` contributes to dedent strip cols via
                // `directive_initial_size_sum`, not here.
                ItemBody::ContainerDirective(..) => {}
                _ => {}
            }
        }
        col
    }

    /// Combined container-prefix strip + 2-column indent dedent for an
    /// inline MDX expression body. The dedent's tab-stop math depends on
    /// whether each line was strict (prefix matched, column = base_col - 1)
    /// or lazy (no prefix, column = 0), so strip and dedent must share
    /// one walk — separating them loses that per-line info.
    /// Normalize an inline expression body (strip container prefixes + 2-col
    /// dedent) and return it alongside an offset map from normalized byte
    /// offsets to absolute source offsets.
    ///
    /// The map stays empty for single-line bodies (the overwhelmingly common
    /// case), where the normalized text is a verbatim source slice and callers
    /// can add offsets directly; `Vec::new()` does not allocate, so that path
    /// pays nothing. It is populated only when continuation lines diverge from
    /// the source, so a parse error there still resolves to the exact column.
    pub(crate) fn inline_expression_value(
        &self,
        start_ix: usize,
        end_ix: usize,
    ) -> (alloc::string::String, Vec<(usize, usize)>) {
        const INDENT: usize = 2;
        const TAB_SIZE: usize = 4;
        let base_col = self.container_content_col().max(1);
        let bytes = self.text.as_bytes();
        let mut out = alloc::string::String::with_capacity(end_ix - start_ix);
        let mut offset_map: Vec<(usize, usize)> = Vec::new();
        let mut pos = start_ix;

        let line_end = memchr::memchr2(b'\n', b'\r', &bytes[pos..end_ix])
            .map(|i| pos + i)
            .unwrap_or(end_ix);
        out.push_str(&self.text[pos..line_end]);
        pos = line_end;

        while pos < end_ix {
            if bytes[pos] == b'\r' {
                out.push('\r');
                pos += 1;
            }
            if pos < end_ix && bytes[pos] == b'\n' {
                out.push('\n');
                pos += 1;
            }
            if pos >= end_ix {
                break;
            }

            let (post_prefix_col, partial_spaces) = if self.tree.spine_len() == 0 {
                (0usize, 0usize)
            } else {
                let mut ls = LineStart::new(&bytes[pos..end_ix]);
                let matched = scan_containers(&self.tree, &mut ls, self.options);
                pos += ls.bytes_scanned();
                let partial = ls.remaining_space();
                // Strict: prefix matched, post-prefix column = base_col - 1.
                // Lazy: no match, line starts at column 0.
                let col = if matched == self.tree.spine_len() {
                    base_col - 1
                } else {
                    0
                };
                (col, partial)
            };
            for _ in 0..partial_spaces {
                out.push(' ');
            }

            // `column` tracks the absolute source column (0-indexed) so
            // tab-stop math is correct.
            let mut stripped = 0usize;
            let mut column = post_prefix_col;
            while pos < end_ix && stripped < INDENT {
                let b = bytes[pos];
                if b == b' ' {
                    stripped += 1;
                    column += 1;
                    pos += 1;
                } else if b == b'\t' {
                    let next_col = (column / TAB_SIZE + 1) * TAB_SIZE;
                    let tab_width = next_col - column;
                    let to_strip = (INDENT - stripped).min(tab_width);
                    stripped += to_strip;
                    for _ in 0..(tab_width - to_strip) {
                        out.push(' ');
                    }
                    column = next_col;
                    pos += 1;
                } else {
                    break;
                }
            }

            // `pos` now sits at the start of this continuation line's content,
            // which is copied verbatim. Record the map entry so an error here
            // resolves to source; the base entry covers the first line.
            if offset_map.is_empty() {
                offset_map.push((0, start_ix));
            }
            offset_map.push((out.len(), pos));

            let line_end = memchr::memchr2(b'\n', b'\r', &bytes[pos..end_ix])
                .map(|i| pos + i)
                .unwrap_or(end_ix);
            out.push_str(&self.text[pos..line_end]);
            pos = line_end;
        }
        (out, offset_map)
    }

    /// Strip container prefixes from continuation lines in a raw text span.
    /// Returns the original text if not inside a container.
    pub(crate) fn strip_container_prefixes(
        &self,
        start_ix: usize,
        end_ix: usize,
    ) -> alloc::borrow::Cow<'_, str> {
        if self.tree.spine_len() == 0 {
            return alloc::borrow::Cow::Borrowed(&self.text[start_ix..end_ix]);
        }

        let bytes = self.text.as_bytes();
        let mut result = alloc::string::String::new();
        let mut pos = start_ix;

        let line_end = memchr::memchr2(b'\n', b'\r', &bytes[pos..end_ix])
            .map(|i| pos + i)
            .unwrap_or(end_ix);
        result.push_str(&self.text[pos..line_end]);
        pos = line_end;

        while pos < end_ix {
            if bytes[pos] == b'\r' {
                result.push('\r');
                pos += 1;
            }
            if pos < end_ix && bytes[pos] == b'\n' {
                result.push('\n');
                pos += 1;
            }

            if pos >= end_ix {
                break;
            }

            let mut ls = LineStart::new(&bytes[pos..]);
            let _ = scan_containers(&self.tree, &mut ls, self.options);
            pos += ls.bytes_scanned();
            // When the container prefix partially consumes a tab (e.g. a
            // list with 3-col indent over a tab-indented continuation), the
            // leftover columns are preserved by remark as literal spaces.
            // Mirror that so downstream position mapping stays consistent.
            for _ in 0..ls.remaining_space() {
                result.push(' ');
            }

            let line_end = memchr::memchr2(b'\n', b'\r', &bytes[pos..end_ix])
                .map(|i| pos + i)
                .unwrap_or(end_ix);
            result.push_str(&self.text[pos..line_end]);
            pos = line_end;
        }

        alloc::borrow::Cow::Owned(result)
    }
}

use crate::{
    parse::{scan_containers, JsxAttr, JsxElementData},
    scanners::LineStart,
};

impl<'a, 'b> FirstPass<'a, 'b> {
    /// Build a closure that checks whether a line starts with the expected
    /// container prefix. Returns the number of prefix bytes to skip, or `None`
    /// for a lazy continuation.
    pub(crate) fn make_container_line_check(&self) -> impl Fn(&[u8]) -> Option<usize> + '_ {
        move |line_bytes: &[u8]| {
            let mut ls = LineStart::new(line_bytes);
            let matched = scan_containers(&self.tree, &mut ls, self.options);
            if matched == self.tree.spine_len() {
                Some(ls.bytes_scanned())
            } else {
                None
            }
        }
    }

    /// Scan for a flow-level MDX block (expression or JSX), handling container
    /// prefixes on continuation lines. Delegates to the scanner directly when
    /// not inside a container; uses the container-aware variant otherwise.
    pub(crate) fn scan_mdx_flow_in_container(
        &self,
        ix: usize,
        scanner: impl Fn(&[u8], Option<ContainerLineCheck<'_>>) -> Option<usize>,
    ) -> Option<usize> {
        self.scan_mdx_flow_in_container_bytes(&self.text.as_bytes()[ix..], scanner)
    }

    /// Same as `scan_mdx_flow_in_container` but takes the byte slice directly.
    /// Used where the slice doesn't start at a known `ix` in `self.text` (e.g.
    /// paragraph-interrupt probes where `bytes` is already container-stripped).
    pub(crate) fn scan_mdx_flow_in_container_bytes(
        &self,
        bytes: &[u8],
        scanner: impl Fn(&[u8], Option<ContainerLineCheck<'_>>) -> Option<usize>,
    ) -> Option<usize> {
        if self.tree.spine_len() == 0 {
            return scanner(bytes, None);
        }

        let check = self.make_container_line_check();
        scanner(bytes, Some(&check))
    }
}

/// Return the 1-indexed column of `bytes[pos]` by walking back to the most
/// recent line start. Tabs in the preceding indent are expanded to 4-column
/// tab stops, matching micromark.
pub(crate) fn column_at(bytes: &[u8], pos: usize) -> usize {
    const TAB_WIDTH: usize = 4;
    let mut line_start = pos;
    while line_start > 0 && bytes[line_start - 1] != b'\n' && bytes[line_start - 1] != b'\r' {
        line_start -= 1;
    }
    let mut col: usize = 1;
    let mut i = line_start;
    while i < pos {
        if bytes[i] == b'\t' {
            col += TAB_WIDTH - ((col - 1) % TAB_WIDTH);
        } else {
            col += 1;
        }
        i += 1;
    }
    col
}

/// Parse a raw JSX tag string into structured `JsxElementData`, given the
/// 1-indexed column where the container's content begins. Multi-line JSX
/// attribute expressions need to strip `(column - 1) + indentSize` columns from
/// each continuation line to match remark's normalized output.
pub(crate) fn parse_jsx_tag_with_column<'a>(
    raw: &'a str,
    container_content_col: usize,
    extra_strip_cols: usize,
) -> JsxElementData<'a> {
    let s = raw.trim();

    if let Some(rest) = s.strip_prefix("</") {
        let name = extract_tag_name(rest.trim_start()).into_owned();
        return JsxElementData {
            name: name.into(),
            attrs: Vec::new(),
            raw: raw.into(),
            is_closing: true,
            is_self_closing: false,
        };
    }

    // Self-closing: a `/` precedes the closing `>`, possibly separated by
    // ASCII whitespace (`<g/\n>`, `<utj/ >`). The simple `ends_with("/>")`
    // would miss those cases and route the tag through the opening-tag arm,
    // which then errors because no matching close tag exists.
    let ends_self_close = {
        let bytes = s.as_bytes();
        if bytes.last() == Some(&b'>') {
            let mut j = bytes.len() - 1;
            while j > 0 && matches!(bytes[j - 1], b' ' | b'\t' | b'\n' | b'\r') {
                j -= 1;
            }
            j > 0 && bytes[j - 1] == b'/'
        } else {
            false
        }
    };

    // Extract name, skip leading '<'
    let name = extract_tag_name(&s[1..]);

    // Search only the children region, past the opening tag's `>`: a `</Name>` inside an
    // attribute value (e.g. a template-literal prop that shows the component's
    // own usage) is text, so it must not pair with the open tag.
    let children = &s[scan_mdx_jsx_tag_end(s.as_bytes()).unwrap_or(s.len())..];
    let is_self_contained = if !name.is_empty() {
        let close_tag = alloc::format!("</{name}>");
        children.contains(&*close_tag)
    } else {
        children.contains("</>")
    };

    let is_self_closing = ends_self_close || is_self_contained;

    let attrs = parse_jsx_attrs(s, container_content_col, extra_strip_cols);

    JsxElementData {
        name: name.into_owned().into(),
        attrs,
        raw: raw.into(),
        is_closing: false,
        is_self_closing,
    }
}

fn extract_tag_name(s: &str) -> alloc::borrow::Cow<'_, str> {
    use alloc::borrow::Cow;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && is_jsx_name_continue(bytes, i) {
        i += char_len_utf8(bytes[i]);
    }
    let primary_end = i;

    // Peek past whitespace for a `.` (member) or `:` (namespace) followed
    // (after more optional whitespace) by another name-start. mdx-js / the
    // micromark JSX scanner permits this — `<a :b/>` is a tag with name
    // `a:b`. If the peek fails, the primary segment is the full name.
    let mut j = primary_end;
    let mut saw_namespace = false;
    let mut owned: Option<alloc::string::String> = None;
    loop {
        let save = j;
        while j < bytes.len() && is_mdx_unicode_whitespace(bytes, j) {
            j += char_len_utf8(bytes[j]);
        }
        if j >= bytes.len() {
            return Cow::Borrowed(&s[..primary_end]);
        }
        let sep = bytes[j];
        let is_member = sep == b'.';
        let is_namespace = sep == b':';
        if !is_member && !is_namespace {
            return owned.map_or_else(|| Cow::Borrowed(&s[..primary_end]), Cow::Owned);
        }
        if is_namespace && saw_namespace {
            return owned.map_or_else(|| Cow::Borrowed(&s[..primary_end]), Cow::Owned);
        }
        if is_namespace {
            saw_namespace = true;
        }
        j += 1;
        let after_sep = j;
        while j < bytes.len() && is_mdx_unicode_whitespace(bytes, j) {
            j += char_len_utf8(bytes[j]);
        }
        if j >= bytes.len() || !is_jsx_name_start(bytes, j) {
            let _ = save;
            return owned.map_or_else(|| Cow::Borrowed(&s[..primary_end]), Cow::Owned);
        }
        let name_chunk_start = j;
        j += char_len_utf8(bytes[j]);
        while j < bytes.len() && is_jsx_name_continue(bytes, j) {
            j += char_len_utf8(bytes[j]);
        }
        let acc = owned.get_or_insert_with(|| s[..primary_end].into());
        acc.push(sep as char);
        let _ = after_sep;
        acc.push_str(&s[name_chunk_start..j]);
    }
}

/// Extract the opening tag portion (up to the first unbalanced `>`).
///
/// Attribute expressions `{...}` are skipped whole via the JS-aware
/// `scan_mdx_expression_end`, so quotes inside a regex (`ins={/x="y"/}`) don't
/// desync the search for the tag's closing `>` — the bug a naive
/// quote/brace tracker hit. Any `>` reached here is therefore outside all
/// braces. Attribute string values (`lang="a > b"`) are HTML-like (no
/// backslash escapes), matching `parse_jsx_attrs`.
fn extract_opening_tag(text: &str) -> &str {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        match bytes[i] {
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                while i < len && bytes[i] != quote {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            b'{' => {
                i += scan_mdx_expression_end(&bytes[i..], false).unwrap_or(1);
            }
            b'>' => return &text[..=i],
            _ => i += 1,
        }
    }
    text
}

fn parse_jsx_attrs<'a>(
    text: &'a str,
    container_content_col: usize,
    extra_strip_cols: usize,
) -> Vec<JsxAttr<'a>> {
    let tag = extract_opening_tag(text);
    let bytes = tag.as_bytes();
    let len = bytes.len();

    let mut attrs = Vec::new();
    let mut i = 1;

    if i < len && bytes[i] == b'/' {
        i += 1;
    }

    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    // Use the shared JSX identifier rules (which know about
    // `$` and Unicode `is_id_start` / `is_id_continue`) and additionally
    // accept the JSX tag-name separators `.` (member) and `:` (namespace).
    // micromark also tolerates whitespace AROUND `.` and `:` (`<a :b/>`
    // → name `a:b`), so peek past whitespace before deciding the name has
    // ended.
    let mut saw_namespace = false;
    loop {
        while i < len && is_jsx_name_continue(bytes, i) {
            i += char_len_utf8(bytes[i]);
        }
        let save = i;
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            i = save;
            break;
        }
        match bytes[i] {
            b':' if !saw_namespace => {
                saw_namespace = true;
                i += 1;
            }
            b'.' if !saw_namespace => {
                i += 1;
            }
            _ => {
                i = save;
                break;
            }
        }
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len || !is_jsx_name_start(bytes, i) {
            // Separator without a following name. Bail out — leave i past
            // the separator; the attribute loop will surface this as an
            // invalid attribute name, matching mdx-js's error mode.
            break;
        }
        i += char_len_utf8(bytes[i]);
    }

    loop {
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }
        if bytes[i] == b'>' || (bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'>') {
            break;
        }

        if bytes[i] == b'{' {
            let end = i + scan_mdx_expression_end(&bytes[i..], false).unwrap_or(len - i);
            let start = i + 1;
            let raw_value = &tag[start..end.saturating_sub(1)];
            let lead_ws = raw_value.len() - raw_value.trim_start().len();
            let value = raw_value.trim();
            let val_start = start + lead_ws;
            i = end;
            attrs.push(JsxAttr::Spread(
                value.into(),
                val_start,
                val_start + value.len(),
            ));
            continue;
        }

        // Attribute name. Use the shared JSX identifier rules (which include
        // `$` and Unicode identifier chars), plus `:` for namespace separators
        // (e.g. `xlink:href`).
        let name_start = i;
        while i < len {
            if bytes[i] == b':' {
                i += 1;
            } else if is_jsx_name_continue(bytes, i) {
                i += char_len_utf8(bytes[i]);
            } else {
                break;
            }
        }
        if i == name_start {
            i += 1;
            continue;
        }
        let name = &tag[name_start..i];

        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        if i < len && bytes[i] == b'=' {
            i += 1;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= len {
                attrs.push(JsxAttr::Boolean(name.into()));
                continue;
            }
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let q = bytes[i];
                i += 1;
                let val_start = i;
                // JSX attribute strings are HTML-like: no backslash escapes.
                while i < len && bytes[i] != q {
                    i += 1;
                }
                let raw_value = &tag[val_start..i];
                if i < len {
                    i += 1;
                }
                let value = strip_attr_continuation_indent(raw_value);
                let decoded = decode_attr_entities(value.as_ref());
                attrs.push(JsxAttr::Literal(name.into(), decoded.into_owned().into()));
            } else if bytes[i] == b'{' {
                let end = i + scan_mdx_expression_end(&bytes[i..], false).unwrap_or(len - i);
                let val_start = i + 1;
                let val_end = end.saturating_sub(1);
                i = end;
                let value = &tag[val_start..val_end];
                let normalized = if value.contains('\n') || value.contains('\r') {
                    alloc::borrow::Cow::Owned(strip_expression_indent(
                        value,
                        container_content_col,
                        extra_strip_cols,
                    ))
                } else {
                    alloc::borrow::Cow::Borrowed(value)
                };
                attrs.push(JsxAttr::Expression(
                    name.into(),
                    normalized.into_owned().into(),
                    val_start,
                    val_end,
                ));
            } else {
                attrs.push(JsxAttr::Boolean(name.into()));
            }
        } else {
            attrs.push(JsxAttr::Boolean(name.into()));
        }
    }

    attrs
}

/// Validate JSX attribute expression bodies (`x={…}`) and spread bodies
/// (`{...x}`) via oxc, mirroring what mdx-js does with acorn at parse time.
/// Without this, only the brace-counting scanner runs on these — garbage
/// like `<a x={1 +}/>` survives until JS emit.
///
/// Bodies are validated against their verbatim slice of `tag` (not the
/// indent-normalized copy stored on the attribute), so an oxc error offset is
/// already in `tag` coordinates. `resolve`, which maps any tag byte offset to
/// the original source, then yields the exact line/column of the offending
/// token rather than the element's opening `<`.
pub(crate) fn validate_jsx_expressions(
    tag: &str,
    attrs: &[JsxAttr<'_>],
    resolve: impl Fn(usize) -> usize,
    allocator: &mut Allocator,
    mdx_errors: &mut Vec<(usize, alloc::string::String)>,
) {
    for attr in attrs {
        // Byte range of the JS body within `tag`. Spreads validate only the
        // operand, past the leading `...`.
        let (body, body_offset) = match attr {
            JsxAttr::Expression(_, _, start, end) => (&tag[*start..*end], *start),
            JsxAttr::Spread(_, start, end) => match tag[*start..*end].strip_prefix("...") {
                Some(operand) => (operand, *end - operand.len()),
                None => continue,
            },
            _ => continue,
        };
        if let Some((err_offset, detail)) = validate_expression_body(body, allocator) {
            mdx_errors.push((
                resolve(body_offset + err_offset),
                alloc::format!("Could not parse expression with oxc: {detail}"),
            ));
        }
    }
}
