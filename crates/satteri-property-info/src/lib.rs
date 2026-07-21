//! HTML/SVG attribute ↔ hast-property name mapping plus value-coercion hints.
//! Table data is ported from [`property-information`].
//!
//! A single table per schema serves both directions: each name is keyed in
//! both its attribute and property forms, so attribute→property
//! ([`find_property`], parsing) reads the property column and
//! property→attribute ([`property_to_attribute`], rendering) reads the
//! attribute column.
//!
//! [`property-information`]: https://github.com/wooorm/property-information

use std::borrow::Cow;

mod tables;
use tables::{HTML_TABLE, SVG_TABLE};

type Table = &'static [(&'static str, &'static str, &'static str, u8)];

/// How an attribute's string value is coerced into a hast property value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropKind {
    /// Plain string (also `booleanish`, which hast keeps as a string).
    String,
    /// Present → `true`, regardless of value.
    Boolean,
    /// Empty value → `true`, otherwise the string.
    OverloadedBoolean,
    /// Numeric value → number, otherwise the string.
    Number,
    /// Whitespace-split list.
    SpaceSeparated,
    /// Comma-split list.
    CommaSeparated,
    /// Comma/whitespace-split list.
    CommaOrSpaceSeparated,
    /// Comma-split list whose numeric items become numbers (`coords`).
    NumberCommaSeparated,
}

impl PropKind {
    fn from_class(class: u8) -> Self {
        match class {
            1 => PropKind::Boolean,
            2 => PropKind::OverloadedBoolean,
            3 => PropKind::Number,
            4 => PropKind::SpaceSeparated,
            5 => PropKind::CommaSeparated,
            6 => PropKind::CommaOrSpaceSeparated,
            7 => PropKind::NumberCommaSeparated,
            _ => PropKind::String,
        }
    }
}

fn table(in_svg: bool) -> Table {
    if in_svg {
        SVG_TABLE
    } else {
        HTML_TABLE
    }
}

/// Lowercase `name` for a table lookup; borrows when already lowercase.
fn normalize(name: &str) -> Cow<'_, str> {
    if name.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

fn lookup(table: Table, key: &str) -> Option<usize> {
    table.binary_search_by(|(k, ..)| (*k).cmp(key)).ok()
}

/// Resolve an HTML/SVG attribute name to its hast property name and value
/// coercion. Known attributes come from the table; `data-*` names are
/// camel-cased; unknown names pass through.
pub fn find_property(local: &str, in_svg: bool) -> (Cow<'_, str>, PropKind) {
    let t = table(in_svg);
    let key = normalize(local);
    if let Some(i) = lookup(t, key.as_ref()) {
        return (Cow::Borrowed(t[i].1), PropKind::from_class(t[i].3));
    }
    if let Some(property) = data_to_property(key.as_ref()) {
        return (Cow::Owned(property), PropKind::String);
    }
    // Unknown attribute: the property name is the normalized attribute name.
    (Cow::Owned(key.into_owned()), PropKind::String)
}

/// Convert a hast property name to its serialized HTML/SVG attribute name.
/// `in_svg` selects the SVG schema; unknown properties pass through unchanged.
pub fn property_to_attribute(name: &str, in_svg: bool) -> Cow<'_, str> {
    // Common HTML case: a lowercase name is already its attribute form. Skips
    // the ladder for `href`, `src`, `id`, ...
    if !in_svg && !name.bytes().any(|b| b.is_ascii_uppercase()) {
        return Cow::Borrowed(name);
    }

    if name == "xmlnsXLink" {
        return Cow::Borrowed("xmlns:xlink");
    }
    if let Some(rest) = strip_namespace_prefix(name, "xLink") {
        return Cow::Owned(format_namespace("xlink:", rest));
    }
    if let Some(rest) = strip_namespace_prefix(name, "xml") {
        return Cow::Owned(format_namespace("xml:", rest));
    }
    // ARIA is intentionally not kebab-cased between words: `ariaValueNow` →
    // `aria-valuenow`, not `aria-value-now`. ARIA spec convention; differs from
    // the data-* case below.
    if let Some(rest) = strip_namespace_prefix(name, "aria") {
        return Cow::Owned(format_namespace("aria-", rest));
    }
    // Schema lookup must beat the generic `data-*` fallback: `dataType` is a
    // real SVG attribute (→ `datatype`), not a custom `data-type`.
    if in_svg {
        if let Some(attr) = attribute_of(name, true) {
            return Cow::Borrowed(attr);
        }
    }
    if let Some(rest) = strip_namespace_prefix(name, "data") {
        return Cow::Owned(format_data_attribute(rest));
    }
    if in_svg {
        return Cow::Borrowed(name);
    }
    if let Some(attr) = attribute_of(name, false) {
        return Cow::Borrowed(attr);
    }
    Cow::Borrowed(name)
}

/// Reverse lookup: the attribute for a known hast property, or `None` for
/// unknown / custom properties (which pass through unchanged).
fn attribute_of(name: &str, in_svg: bool) -> Option<&'static str> {
    let t = table(in_svg);
    lookup(t, normalize(name).as_ref()).map(|i| t[i].2)
}

/// Turn a `data-*` attribute name into its hast property name
/// (`data-foo-bar` → `dataFooBar`, `data-a-b-c` → `dataABC`,
/// `data-x-1` → `dataX-1`). Expects `name` already lowercased; returns `None`
/// for names that are not valid data attributes.
fn data_to_property(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    if bytes.len() <= 4 || &bytes[..4] != b"data" || bytes[4] != b'-' {
        return None;
    }
    if !name[4..]
        .bytes()
        .all(|b| b == b'-' || b == b'.' || b == b':' || b == b'_' || b.is_ascii_alphanumeric())
    {
        return None;
    }
    let rest = &name.as_bytes()[5..];
    let mut camel = String::with_capacity(rest.len());
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == b'-' && rest.get(i + 1).is_some_and(u8::is_ascii_lowercase) {
            camel.push(rest[i + 1].to_ascii_uppercase() as char);
            i += 2;
        } else {
            camel.push(rest[i] as char);
            i += 1;
        }
    }
    // `property = 'data' + rest[0].toUpperCase() + rest.slice(1)`.
    let mut property = String::with_capacity(4 + camel.len());
    property.push_str("data");
    let mut chars = camel.chars();
    if let Some(first) = chars.next() {
        property.extend(first.to_uppercase());
        property.push_str(chars.as_str());
    }
    Some(property)
}

/// Returns the suffix after `prefix` only when the next character is uppercase,
/// so bare words like `datatype` or `arial` don't get namespaced.
fn strip_namespace_prefix<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = name.strip_prefix(prefix)?;
    rest.starts_with(|c: char| c.is_ascii_uppercase())
        .then_some(rest)
}

fn format_namespace(prefix: &str, suffix: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + suffix.len());
    out.push_str(prefix);
    for c in suffix.chars() {
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn format_data_attribute(suffix: &str) -> String {
    let mut out = String::with_capacity(4 + suffix.len() + 4);
    out.push_str("data");
    for c in suffix.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{find_property, property_to_attribute, PropKind};

    fn html(name: &str) -> std::borrow::Cow<'_, str> {
        property_to_attribute(name, false)
    }

    fn svg(name: &str) -> std::borrow::Cow<'_, str> {
        property_to_attribute(name, true)
    }

    #[test]
    fn html_special_cases() {
        assert_eq!(html("className"), "class");
        assert_eq!(html("htmlFor"), "for");
        assert_eq!(html("httpEquiv"), "http-equiv");
        assert_eq!(html("acceptCharset"), "accept-charset");
    }

    #[test]
    fn known_html_properties_are_lowercased() {
        assert_eq!(html("srcSet"), "srcset");
        assert_eq!(html("maxLength"), "maxlength");
        assert_eq!(html("minLength"), "minlength");
        assert_eq!(html("readOnly"), "readonly");
        assert_eq!(html("autoPlay"), "autoplay");
        assert_eq!(html("autoFocus"), "autofocus");
        assert_eq!(html("contentEditable"), "contenteditable");
        assert_eq!(html("tabIndex"), "tabindex");
        assert_eq!(html("colSpan"), "colspan");
        assert_eq!(html("rowSpan"), "rowspan");
        assert_eq!(html("crossOrigin"), "crossorigin");
        assert_eq!(html("dateTime"), "datetime");
        assert_eq!(html("charSet"), "charset");
        assert_eq!(html("noValidate"), "novalidate");
        assert_eq!(html("referrerPolicy"), "referrerpolicy");
        assert_eq!(html("inputMode"), "inputmode");
        assert_eq!(html("enterKeyHint"), "enterkeyhint");
        assert_eq!(html("spellCheck"), "spellcheck");
        assert_eq!(html("accessKey"), "accesskey");
        assert_eq!(html("itemProp"), "itemprop");
        assert_eq!(html("imageSrcSet"), "imagesrcset");
        assert_eq!(html("formNoValidate"), "formnovalidate");
    }

    #[test]
    fn event_handlers_are_lowercased() {
        assert_eq!(html("onClick"), "onclick");
        assert_eq!(html("onKeyDown"), "onkeydown");
        assert_eq!(html("onMouseOver"), "onmouseover");
        assert_eq!(html("onCanPlayThrough"), "oncanplaythrough");
    }

    #[test]
    fn legacy_properties_are_lowercased() {
        assert_eq!(html("bgColor"), "bgcolor");
        assert_eq!(html("cellPadding"), "cellpadding");
        assert_eq!(html("vAlign"), "valign");
        assert_eq!(html("longDesc"), "longdesc");
    }

    #[test]
    fn aria_lowercases_suffix_without_inner_hyphens() {
        assert_eq!(html("ariaHidden"), "aria-hidden");
        assert_eq!(html("ariaLive"), "aria-live");
        // ARIA attributes do NOT get inner hyphens between words.
        assert_eq!(html("ariaValueNow"), "aria-valuenow");
        assert_eq!(html("ariaActiveDescendant"), "aria-activedescendant");
        // ARIA works the same in SVG context.
        assert_eq!(svg("ariaHidden"), "aria-hidden");
        assert_eq!(svg("ariaValueNow"), "aria-valuenow");
    }

    #[test]
    fn data_kebab_cases_suffix() {
        assert_eq!(html("dataLanguage"), "data-language");
        assert_eq!(html("dataFooBar"), "data-foo-bar");
        // data-* works the same in SVG context.
        assert_eq!(svg("dataLanguage"), "data-language");
    }

    #[test]
    fn xlink_namespaces_lowercased_suffix() {
        assert_eq!(html("xLinkHref"), "xlink:href");
        assert_eq!(html("xLinkActuate"), "xlink:actuate");
        assert_eq!(html("xLinkArcRole"), "xlink:arcrole");
        assert_eq!(html("xLinkType"), "xlink:type");
        // xlink works the same in SVG context (it's where it actually belongs).
        assert_eq!(svg("xLinkHref"), "xlink:href");
    }

    #[test]
    fn xml_namespaces_lowercased_suffix() {
        assert_eq!(html("xmlLang"), "xml:lang");
        assert_eq!(html("xmlBase"), "xml:base");
        assert_eq!(html("xmlSpace"), "xml:space");
        assert_eq!(svg("xmlLang"), "xml:lang");
    }

    #[test]
    fn xmlns_special_cases() {
        assert_eq!(html("xmlnsXLink"), "xmlns:xlink");
        assert_eq!(svg("xmlnsXLink"), "xmlns:xlink");
    }

    #[test]
    fn unknown_properties_pass_through() {
        assert_eq!(html("foo"), "foo");
        assert_eq!(html("my-custom"), "my-custom");
        // Property that does not start with an uppercase after the prefix is unchanged.
        assert_eq!(html("datatype"), "datatype");
        assert_eq!(html("arial"), "arial");
        assert_eq!(html("dangerouslySetInnerHTML"), "dangerouslySetInnerHTML");
        assert_eq!(html("customProp"), "customProp");
    }

    #[test]
    fn svg_kebab_cased_attributes() {
        assert_eq!(svg("fillRule"), "fill-rule");
        assert_eq!(svg("clipRule"), "clip-rule");
        assert_eq!(svg("strokeWidth"), "stroke-width");
        assert_eq!(svg("strokeLineCap"), "stroke-linecap");
        assert_eq!(svg("strokeLineJoin"), "stroke-linejoin");
        assert_eq!(svg("strokeDashArray"), "stroke-dasharray");
        assert_eq!(svg("strokeDashOffset"), "stroke-dashoffset");
        assert_eq!(svg("alignmentBaseline"), "alignment-baseline");
        assert_eq!(svg("dominantBaseline"), "dominant-baseline");
        assert_eq!(svg("textAnchor"), "text-anchor");
        assert_eq!(svg("transformOrigin"), "transform-origin");
        assert_eq!(svg("vectorEffect"), "vector-effect");
        assert_eq!(svg("xHeight"), "x-height");
        assert_eq!(svg("panose1"), "panose-1");
    }

    #[test]
    fn svg_lowercased_attributes() {
        assert_eq!(svg("crossOrigin"), "crossorigin");
        assert_eq!(svg("hrefLang"), "hreflang");
        assert_eq!(svg("referrerPolicy"), "referrerpolicy");
        assert_eq!(svg("tabIndex"), "tabindex");
        assert_eq!(svg("typeOf"), "typeof");
        assert_eq!(svg("dataType"), "datatype");
        assert_eq!(svg("playbackOrder"), "playbackorder");
        assert_eq!(svg("timelineBegin"), "timelinebegin");
        assert_eq!(svg("onClick"), "onclick");
    }

    #[test]
    fn svg_case_preserved_attributes() {
        // These appear in SVG's `properties` map but NOT in the `attributes`
        // map, so they're case-preserved.
        assert_eq!(svg("viewBox"), "viewBox");
        assert_eq!(svg("preserveAspectRatio"), "preserveAspectRatio");
        assert_eq!(svg("gradientUnits"), "gradientUnits");
        assert_eq!(svg("gradientTransform"), "gradientTransform");
        assert_eq!(svg("patternUnits"), "patternUnits");
        assert_eq!(svg("patternTransform"), "patternTransform");
        assert_eq!(svg("clipPathUnits"), "clipPathUnits");
        assert_eq!(svg("maskUnits"), "maskUnits");
        assert_eq!(svg("maskContentUnits"), "maskContentUnits");
        assert_eq!(svg("markerUnits"), "markerUnits");
        assert_eq!(svg("primitiveUnits"), "primitiveUnits");
        assert_eq!(svg("filterUnits"), "filterUnits");
        assert_eq!(svg("baseFrequency"), "baseFrequency");
        assert_eq!(svg("numOctaves"), "numOctaves");
        assert_eq!(svg("stdDeviation"), "stdDeviation");
        assert_eq!(svg("attributeName"), "attributeName");
        assert_eq!(svg("attributeType"), "attributeType");
        assert_eq!(svg("repeatCount"), "repeatCount");
        assert_eq!(svg("keyTimes"), "keyTimes");
        assert_eq!(svg("keySplines"), "keySplines");
        assert_eq!(svg("keyPoints"), "keyPoints");
        assert_eq!(svg("xChannelSelector"), "xChannelSelector");
        assert_eq!(svg("yChannelSelector"), "yChannelSelector");
        assert_eq!(svg("zoomAndPan"), "zoomAndPan");
        // Already-lowercase SVG attrs (case-preserved trivially).
        assert_eq!(svg("width"), "width");
        assert_eq!(svg("height"), "height");
        assert_eq!(svg("fill"), "fill");
        assert_eq!(svg("d"), "d");
    }

    #[test]
    fn svg_unknown_passes_through() {
        // Custom SVG-namespace attrs we don't know about: pass through.
        assert_eq!(svg("customAttr"), "customAttr");
        assert_eq!(svg("foo"), "foo");
    }

    #[test]
    fn html_only_names_in_svg_context_pass_through() {
        // `htmlFor` / `httpEquiv` / `acceptCharset` are HTML-only; in SVG the
        // schema doesn't know them, so they pass through unchanged.
        assert_eq!(svg("htmlFor"), "htmlFor");
        assert_eq!(svg("httpEquiv"), "httpEquiv");
        assert_eq!(svg("acceptCharset"), "acceptCharset");
    }

    #[test]
    fn html_only_lowercased_in_svg_context_pass_through() {
        // SVG does not lowercase arbitrary HTML props; only those in its
        // explicit table get rewritten.
        assert_eq!(svg("srcSet"), "srcSet");
        assert_eq!(svg("maxLength"), "maxLength");
        assert_eq!(svg("readOnly"), "readOnly");
        assert_eq!(svg("contentEditable"), "contentEditable");
    }

    #[test]
    fn find_property_forward_direction() {
        assert_eq!(
            find_property("class", false),
            ("className".into(), PropKind::SpaceSeparated)
        );
        assert_eq!(
            find_property("href", false),
            ("href".into(), PropKind::String)
        );
        assert_eq!(
            find_property("disabled", false),
            ("disabled".into(), PropKind::Boolean)
        );
        assert_eq!(
            find_property("download", false),
            ("download".into(), PropKind::OverloadedBoolean)
        );
        assert_eq!(
            find_property("tabindex", false),
            ("tabIndex".into(), PropKind::Number)
        );
        assert_eq!(
            find_property("accept", false),
            ("accept".into(), PropKind::CommaSeparated)
        );
        assert_eq!(
            find_property("data-foo-bar", false),
            ("dataFooBar".into(), PropKind::String)
        );
        assert_eq!(
            find_property("aria-label", false),
            ("ariaLabel".into(), PropKind::String)
        );
        assert_eq!(
            find_property("data-x-1", false),
            ("dataX-1".into(), PropKind::String)
        );
        assert_eq!(
            find_property("unknownattr", false),
            ("unknownattr".into(), PropKind::String)
        );
        assert_eq!(
            find_property("viewBox", true),
            ("viewBox".into(), PropKind::String)
        );
    }

    #[test]
    fn reverse_direction_matches_the_table_for_every_entry() {
        // For every row, `property_to_attribute(property)` must reproduce the
        // stored attribute — the algorithmic aria/xlink/xml branches included.
        for (table, in_svg) in [(super::HTML_TABLE, false), (super::SVG_TABLE, true)] {
            for (_, property, attribute, _) in table {
                assert_eq!(
                    &property_to_attribute(property, in_svg),
                    attribute,
                    "{} property {property}",
                    if in_svg { "svg" } else { "html" }
                );
            }
        }
    }
}
