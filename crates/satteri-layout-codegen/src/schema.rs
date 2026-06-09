//! The node registry: the single source of truth for every MDAST and HAST node
//! type. Each [`Node`] declares its tree, tag, Rust enum variant, AST name, and
//! (for fixed-field leaf types) its `type_data` layout.
//!
//! Everything downstream is generated from this one table:
//!   * the `MdastNodeType` / `HastNodeType` enums (`generated/node_types.rs`),
//!   * the TS name maps and visitor keys (`generated/node-types.ts`),
//!   * the walk serializers + layout decoders (`walk_type_data.rs`, `layout.ts`),
//!   * compile-time layout assertions (`assert_layouts.rs`).
//!
//! Add a node here once; every list that used to repeat it is regenerated.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tree {
    Mdast,
    Hast,
}

/// How a field crosses the walk wire (Rust -> JS).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// `StringRef` resolved to an inline string with a `u16` length prefix.
    Str16,
    /// `StringRef` resolved to an inline string with a `u32` length prefix
    /// (for `value` fields, which can be large).
    Str32,
    /// A single stored byte, copied verbatim.
    U8,
}

/// How the decoded value is surfaced to JS.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Js {
    /// String; an empty string stays `""`.
    Str,
    /// String; an empty string becomes `null` (e.g. `title`, `lang`).
    StrNull,
    /// Numeric byte (e.g. `depth`).
    Num,
    /// Byte mapped through an enum value list (e.g. `referenceType`).
    Enum(&'static [&'static str]),
    /// Present on the wire but not assigned to the JS node (e.g. the kind byte
    /// on `footnoteReference`, which the mdast spec does not expose).
    Skip,
    /// A constant byte written on encode, absent from the wire and JS (e.g. the
    /// `fence_char` on `code`, which only affects rendering).
    Const(u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// JS property name (ignored when `js_kind` is [`Js::Skip`]).
    pub js: &'static str,
    /// Byte offset of the field within the node's `type_data`.
    pub offset: usize,
    pub wire: Wire,
    pub js_kind: Js,
    /// Fallback byte when `type_data` is shorter than `offset` (U8 fields only).
    pub u8_default: u8,
    /// Restore MDX phantom-space sentinels on decode (expression/JSX values).
    pub phantom: bool,
}

pub struct Node {
    /// Kept for readability of the registry; the two tree tables are already
    /// split, so the generators don't read it back.
    #[allow(dead_code)]
    pub tree: Tree,
    pub tag: u8,
    /// Rust enum variant identifier (may differ from `name`, e.g. HAST
    /// `MdxJsxElement` whose name is `"mdxJsxFlowElement"`).
    pub variant: &'static str,
    /// Canonical AST/JS name.
    pub name: &'static str,
    /// Fixed `type_data` fields. Empty for container / no-data nodes and for
    /// `custom` nodes (whose variable-length codec stays hand-written).
    pub fields: &'static [Field],
    /// Variable-length `type_data` handled by hand-written code on both sides.
    pub custom: bool,
}

/// One fixed-field wire layout shared by every tag with the same field list.
pub struct Layout {
    pub tags: Vec<u8>,
    pub fields: &'static [Field],
}

/// A codec struct whose memory layout the generated assertions pin down.
pub struct ArenaStruct {
    pub rust: &'static str,
    pub size: usize,
    pub offsets: &'static [(&'static str, usize)],
}

const fn s16(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str16,
        js_kind: Js::Str,
        u8_default: 0,
        phantom: false,
    }
}
const fn s16n(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str16,
        js_kind: Js::StrNull,
        u8_default: 0,
        phantom: false,
    }
}
const fn s32(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str32,
        js_kind: Js::Str,
        u8_default: 0,
        phantom: false,
    }
}
/// A `value` field that carries MDX phantom-space sentinels.
const fn s32p(js: &'static str, offset: usize) -> Field {
    Field {
        js,
        offset,
        wire: Wire::Str32,
        js_kind: Js::Str,
        u8_default: 0,
        phantom: true,
    }
}
const fn num(js: &'static str, offset: usize, default: u8) -> Field {
    Field {
        js,
        offset,
        wire: Wire::U8,
        js_kind: Js::Num,
        u8_default: default,
        phantom: false,
    }
}
const fn enum8(js: &'static str, offset: usize, values: &'static [&'static str]) -> Field {
    Field {
        js,
        offset,
        wire: Wire::U8,
        js_kind: Js::Enum(values),
        u8_default: 0,
        phantom: false,
    }
}
const fn skip8(offset: usize) -> Field {
    Field {
        js: "",
        offset,
        wire: Wire::U8,
        js_kind: Js::Skip,
        u8_default: 0,
        phantom: false,
    }
}
/// A constant byte at `offset`, written only on encode (absent from wire/JS).
const fn konst(js: &'static str, offset: usize, value: u8) -> Field {
    Field {
        js,
        offset,
        wire: Wire::U8,
        js_kind: Js::Const(value),
        u8_default: 0,
        phantom: false,
    }
}

const REF_KINDS: &[&str] = &["shortcut", "collapsed", "full"];

const VALUE: &[Field] = &[s32("value", 0)];
const EXPR_VALUE: &[Field] = &[s32p("value", 0)];
const MATH: &[Field] = &[s16n("meta", 0), s32("value", 8)];
const NONE: &[Field] = &[];

use Tree::{Hast, Mdast};

/// A container or no-`type_data` node (no fields, not custom).
const fn c(tree: Tree, tag: u8, variant: &'static str, name: &'static str) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields: NONE,
        custom: false,
    }
}
/// A leaf node with a fixed-field layout.
const fn n(
    tree: Tree,
    tag: u8,
    variant: &'static str,
    name: &'static str,
    fields: &'static [Field],
) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields,
        custom: false,
    }
}
/// A node whose variable-length `type_data` codec stays hand-written.
const fn x(tree: Tree, tag: u8, variant: &'static str, name: &'static str) -> Node {
    Node {
        tree,
        tag,
        variant,
        name,
        fields: NONE,
        custom: true,
    }
}

pub const MDAST_NODES: &[Node] = &[
    c(Mdast, 0, "Root", "root"),
    c(Mdast, 1, "Paragraph", "paragraph"),
    n(Mdast, 2, "Heading", "heading", &[num("depth", 0, 1)]),
    c(Mdast, 3, "ThematicBreak", "thematicBreak"),
    c(Mdast, 4, "Blockquote", "blockquote"),
    x(Mdast, 5, "List", "list"),
    x(Mdast, 6, "ListItem", "listItem"),
    n(Mdast, 7, "Html", "html", VALUE),
    n(
        Mdast,
        8,
        "Code",
        "code",
        &[
            s16n("lang", 0),
            s16n("meta", 8),
            s32("value", 16),
            konst("fence", 24, b'`'),
        ],
    ),
    n(
        Mdast,
        9,
        "Definition",
        "definition",
        &[
            s16("url", 0),
            s16n("title", 8),
            s16("identifier", 16),
            s16("label", 24),
        ],
    ),
    n(Mdast, 10, "Text", "text", VALUE),
    c(Mdast, 11, "Emphasis", "emphasis"),
    c(Mdast, 12, "Strong", "strong"),
    n(Mdast, 13, "InlineCode", "inlineCode", VALUE),
    c(Mdast, 14, "Break", "break"),
    n(
        Mdast,
        15,
        "Link",
        "link",
        &[s16("url", 0), s16n("title", 8)],
    ),
    n(
        Mdast,
        16,
        "Image",
        "image",
        &[s16("url", 0), s16("alt", 8), s16n("title", 16)],
    ),
    n(
        Mdast,
        17,
        "LinkReference",
        "linkReference",
        &[
            s16("identifier", 0),
            s16("label", 8),
            enum8("referenceType", 16, REF_KINDS),
        ],
    ),
    n(
        Mdast,
        18,
        "ImageReference",
        "imageReference",
        &[
            s16("identifier", 0),
            s16("label", 8),
            enum8("referenceType", 16, REF_KINDS),
            s16("alt", 20),
        ],
    ),
    n(
        Mdast,
        19,
        "FootnoteDefinition",
        "footnoteDefinition",
        &[s16("identifier", 0), s16("label", 8)],
    ),
    n(
        Mdast,
        20,
        "FootnoteReference",
        "footnoteReference",
        &[s16("identifier", 0), s16("label", 8), skip8(16)],
    ),
    x(Mdast, 21, "Table", "table"),
    c(Mdast, 22, "TableRow", "tableRow"),
    c(Mdast, 23, "TableCell", "tableCell"),
    c(Mdast, 24, "Delete", "delete"),
    n(Mdast, 25, "Yaml", "yaml", VALUE),
    n(Mdast, 26, "Toml", "toml", VALUE),
    n(Mdast, 27, "Math", "math", MATH),
    // InlineMath shares Math's stored `MathData` (meta@0, value@8) but the mdast
    // spec gives it no `meta`, so only `value` is surfaced.
    n(Mdast, 28, "InlineMath", "inlineMath", &[s32("value", 8)]),
    x(Mdast, 30, "ContainerDirective", "containerDirective"),
    x(Mdast, 31, "LeafDirective", "leafDirective"),
    x(Mdast, 32, "TextDirective", "textDirective"),
    x(Mdast, 100, "MdxJsxFlowElement", "mdxJsxFlowElement"),
    x(Mdast, 101, "MdxJsxTextElement", "mdxJsxTextElement"),
    n(
        Mdast,
        102,
        "MdxFlowExpression",
        "mdxFlowExpression",
        EXPR_VALUE,
    ),
    n(
        Mdast,
        103,
        "MdxTextExpression",
        "mdxTextExpression",
        EXPR_VALUE,
    ),
    n(Mdast, 104, "MdxjsEsm", "mdxjsEsm", EXPR_VALUE),
];

pub const HAST_NODES: &[Node] = &[
    c(Hast, 0, "Root", "root"),
    x(Hast, 1, "Element", "element"),
    c(Hast, 2, "Text", "text"),
    c(Hast, 3, "Comment", "comment"),
    c(Hast, 4, "Doctype", "doctype"),
    c(Hast, 5, "Raw", "raw"),
    x(Hast, 10, "MdxJsxElement", "mdxJsxFlowElement"),
    x(Hast, 11, "MdxJsxTextElement", "mdxJsxTextElement"),
    x(Hast, 12, "MdxFlowExpression", "mdxFlowExpression"),
    x(Hast, 13, "MdxEsm", "mdxjsEsm"),
    x(Hast, 14, "MdxTextExpression", "mdxTextExpression"),
];

/// Group a tree's fixed-field nodes into shared wire layouts (tags with an
/// identical field list collapse to one [`Layout`], in first-seen order).
pub fn layouts(nodes: &[Node]) -> Vec<Layout> {
    let mut out: Vec<Layout> = Vec::new();
    for node in nodes {
        if node.custom || node.fields.is_empty() {
            continue;
        }
        match out.iter_mut().find(|l| l.fields == node.fields) {
            Some(layout) => layout.tags.push(node.tag),
            None => out.push(Layout {
                tags: vec![node.tag],
                fields: node.fields,
            }),
        }
    }
    out
}

pub const MDAST_STRUCTS: &[ArenaStruct] = &[
    ArenaStruct {
        rust: "HeadingData",
        size: 1,
        offsets: &[("depth", 0)],
    },
    ArenaStruct {
        rust: "CodeData",
        size: 28,
        offsets: &[("lang", 0), ("meta", 8), ("value", 16), ("fence_char", 24)],
    },
    ArenaStruct {
        rust: "MathData",
        size: 16,
        offsets: &[("meta", 0), ("value", 8)],
    },
    ArenaStruct {
        rust: "LinkData",
        size: 16,
        offsets: &[("url", 0), ("title", 8)],
    },
    ArenaStruct {
        rust: "ImageData",
        size: 24,
        offsets: &[("url", 0), ("alt", 8), ("title", 16)],
    },
    ArenaStruct {
        rust: "DefinitionData",
        size: 32,
        offsets: &[("url", 0), ("title", 8), ("identifier", 16), ("label", 24)],
    },
    ArenaStruct {
        rust: "ReferenceData",
        size: 20,
        offsets: &[("identifier", 0), ("label", 8), ("reference_kind", 16)],
    },
    ArenaStruct {
        rust: "FootnoteDefinitionData",
        size: 16,
        offsets: &[("identifier", 0), ("label", 8)],
    },
];
