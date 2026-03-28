//! JSX attribute parser: extract attributes from raw JSX source text.
//!
//! Shared between the MDAST parser (stores attributes at parse time) and the
//! HAST converter (reads them back).

/// Parsed JSX attribute — intermediate representation before binary encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsxAttr {
    BooleanProp(String),            // name (no value)
    LiteralProp(String, String),    // name="literal"
    ExpressionProp(String, String), // name={expr}
    Spread(String),                 // {...expr}
}

pub fn parse_jsx_attributes_from_tag(text: &str) -> Vec<JsxAttr> {
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

    while i < len
        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'.' | b'-' | b':' | b'_'))
    {
        i += 1;
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

        // Spread expression
        if bytes[i] == b'{' {
            i += 1;
            let start = i;
            let mut depth = 1i32;
            while i < len && depth > 0 {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    b'\'' | b'"' | b'`' => {
                        let q = bytes[i];
                        i += 1;
                        while i < len && bytes[i] != q {
                            if bytes[i] == b'\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let value = tag[start..i.saturating_sub(1)].trim().to_string();
            attrs.push(JsxAttr::Spread(value));
            continue;
        }

        let name_start = i;
        while i < len
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b':' | b'_'))
        {
            i += 1;
        }
        if i == name_start {
            i += 1;
            continue;
        }
        let name = tag[name_start..i].to_string();

        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        if i < len && bytes[i] == b'=' {
            i += 1;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= len {
                attrs.push(JsxAttr::BooleanProp(name));
                continue;
            }
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let q = bytes[i];
                i += 1;
                let val_start = i;
                while i < len && bytes[i] != q {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                let value = tag[val_start..i].to_string();
                if i < len {
                    i += 1;
                }
                attrs.push(JsxAttr::LiteralProp(name, value));
            } else if bytes[i] == b'{' {
                i += 1;
                let val_start = i;
                let mut depth = 1i32;
                while i < len && depth > 0 {
                    match bytes[i] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        b'\'' | b'"' | b'`' => {
                            let q = bytes[i];
                            i += 1;
                            while i < len && bytes[i] != q {
                                if bytes[i] == b'\\' {
                                    i += 1;
                                }
                                i += 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let value = tag[val_start..i.saturating_sub(1)].to_string();
                attrs.push(JsxAttr::ExpressionProp(name, value));
            } else {
                attrs.push(JsxAttr::BooleanProp(name));
            }
        } else {
            attrs.push(JsxAttr::BooleanProp(name));
        }
    }

    attrs
}

/// Extract the opening tag from JSX source, handling brace/string nesting.
pub fn extract_opening_tag(text: &str) -> &str {
    let mut depth = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut prev = '\0';

    for (i, ch) in text.char_indices() {
        if in_single_quote {
            if ch == '\'' && prev != '\\' {
                in_single_quote = false;
            }
        } else if in_double_quote {
            if ch == '"' && prev != '\\' {
                in_double_quote = false;
            }
        } else if in_backtick {
            if ch == '`' && prev != '\\' {
                in_backtick = false;
            }
        } else {
            match ch {
                '\'' => in_single_quote = true,
                '"' => in_double_quote = true,
                '`' => in_backtick = true,
                '{' => depth += 1,
                '}' => depth -= 1,
                '>' if depth == 0 => return &text[..=i],
                _ => {}
            }
        }
        prev = ch;
    }
    text
}

pub const MDX_ATTR_BOOLEAN_PROP: u8 = 0;
pub const MDX_ATTR_LITERAL_PROP: u8 = 1;
pub const MDX_ATTR_EXPRESSION_PROP: u8 = 2;
pub const MDX_ATTR_SPREAD: u8 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_attrs() {
        let attrs = parse_jsx_attributes_from_tag("<Component />");
        assert!(attrs.is_empty());
    }

    #[test]
    fn parse_boolean_attr() {
        let attrs = parse_jsx_attributes_from_tag("<Component disabled />");
        assert_eq!(attrs, vec![JsxAttr::BooleanProp("disabled".into())]);
    }

    #[test]
    fn parse_literal_attr() {
        let attrs = parse_jsx_attributes_from_tag(r#"<Component foo="bar" />"#);
        assert_eq!(
            attrs,
            vec![JsxAttr::LiteralProp("foo".into(), "bar".into())]
        );
    }

    #[test]
    fn parse_expression_attr() {
        let attrs = parse_jsx_attributes_from_tag("<Component count={42} />");
        assert_eq!(
            attrs,
            vec![JsxAttr::ExpressionProp("count".into(), "42".into())]
        );
    }

    #[test]
    fn parse_spread() {
        let attrs = parse_jsx_attributes_from_tag("<Component {...props} />");
        assert_eq!(attrs, vec![JsxAttr::Spread("...props".into())]);
    }

    #[test]
    fn parse_mixed_attrs() {
        let attrs = parse_jsx_attributes_from_tag(r#"<C a="1" b={2} c {...d} />"#);
        assert_eq!(
            attrs,
            vec![
                JsxAttr::LiteralProp("a".into(), "1".into()),
                JsxAttr::ExpressionProp("b".into(), "2".into()),
                JsxAttr::BooleanProp("c".into()),
                JsxAttr::Spread("...d".into()),
            ]
        );
    }

    #[test]
    fn parse_fragment() {
        let attrs = parse_jsx_attributes_from_tag("<></>");
        assert!(attrs.is_empty());
    }
}
