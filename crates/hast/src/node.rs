//! HAST node types, arena, and builder.

use tryckeri_mdast::StringRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HastNodeType {
    Root = 0,
    Element = 1,
    Text = 2,
    Comment = 3,
    Doctype = 4,
    Raw = 5,                // raw HTML passthrough (from MDAST Html nodes)
    MdxJsxElement = 10,     // MDX JSX flow element (<Component>)
    MdxJsxTextElement = 11, // MDX JSX text element (inline <Component />)
    MdxExpression = 12,     // MDX expression ({expr})
    MdxEsm = 13,            // MDX ESM (import/export)
}

impl HastNodeType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Root),
            1 => Some(Self::Element),
            2 => Some(Self::Text),
            3 => Some(Self::Comment),
            4 => Some(Self::Doctype),
            5 => Some(Self::Raw),
            10 => Some(Self::MdxJsxElement),
            11 => Some(Self::MdxJsxTextElement),
            12 => Some(Self::MdxExpression),
            13 => Some(Self::MdxEsm),
            _ => None,
        }
    }
}

/// An HTML attribute (property) on an element.
#[derive(Debug, Clone, Copy)]
pub struct Property {
    pub name: StringRef,
    pub value: PropertyValue,
}

/// Property value — all strings are `StringRef` into the arena string pool.
///
/// `SpaceSeparated` and `CommaSeparated` store the pre-joined string
/// (e.g. `"foo bar"` or `"a, b"`); splitting is only needed on the JS side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PropertyValue {
    String(StringRef),
    Bool(bool),
    SpaceSeparated(StringRef),
    CommaSeparated(StringRef),
}

impl PropertyValue {
    /// Render to HTML attribute value — returns a `StringRef` into the arena.
    /// For `Bool(true)` / `Bool(false)` returns an empty ref (caller skips).
    pub fn as_string_ref(&self) -> StringRef {
        match self {
            Self::String(r) | Self::SpaceSeparated(r) | Self::CommaSeparated(r) => *r,
            Self::Bool(_) => StringRef::empty(),
        }
    }

    pub fn is_bool_false(&self) -> bool {
        matches!(self, Self::Bool(false))
    }
}

/// All strings are `StringRef` into `HastArena::strings` — zero per-node heap allocations.
#[derive(Debug, Clone, Copy)]
pub struct HastNode {
    pub id: u32,
    pub node_type: HastNodeType,
    pub parent: u32,

    // For Element nodes — StringRef::empty() when not an element.
    pub tag_name: StringRef,
    pub props_start: u32, // index into HastArena::properties
    pub props_count: u32,

    // For Text, Comment, Raw nodes — StringRef::empty() when not applicable.
    pub value: StringRef,

    // Children (for Root, Element):
    pub children_start: u32,
    pub children_count: u32,

    // Source position (optional, from MDAST)
    pub start_line: u32, // 0 = not set
    pub end_line: u32,   // 0 = not set
}

pub struct HastArena {
    pub nodes: Vec<HastNode>,
    pub children: Vec<u32>,
    pub properties: Vec<Property>,
    /// String pool — all tag names, text content, property names/values live here.
    pub strings: String,
}

impl HastArena {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            children: Vec::new(),
            properties: Vec::new(),
            strings: String::new(),
        }
    }

    pub fn alloc_string(&mut self, s: &str) -> StringRef {
        let offset = self.strings.len() as u32;
        let len = s.len() as u32;
        self.strings.push_str(s);
        StringRef::new(offset, len)
    }

    pub fn get_str(&self, r: StringRef) -> &str {
        if r.is_empty() {
            return "";
        }
        let start = r.offset as usize;
        let end = start + r.len as usize;
        &self.strings[start..end]
    }

    pub fn alloc_node(&mut self, node_type: HastNodeType) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(HastNode {
            id,
            node_type,
            parent: 0,
            tag_name: StringRef::empty(),
            props_start: 0,
            props_count: 0,
            value: StringRef::empty(),
            children_start: 0,
            children_count: 0,
            start_line: 0,
            end_line: 0,
        });
        id
    }

    pub fn get_node(&self, id: u32) -> &HastNode {
        &self.nodes[id as usize]
    }

    pub fn get_node_mut(&mut self, id: u32) -> &mut HastNode {
        &mut self.nodes[id as usize]
    }

    pub fn set_children(&mut self, parent_id: u32, child_ids: &[u32]) {
        let start = self.children.len() as u32;
        self.children.extend_from_slice(child_ids);
        let node = self.get_node_mut(parent_id);
        node.children_start = start;
        node.children_count = child_ids.len() as u32;
        for &child_id in child_ids {
            self.nodes[child_id as usize].parent = parent_id;
        }
    }

    pub fn get_children(&self, id: u32) -> &[u32] {
        let node = self.get_node(id);
        &self.children
            [node.children_start as usize..(node.children_start + node.children_count) as usize]
    }

    pub fn add_properties(&mut self, node_id: u32, props: &[Property]) {
        let start = self.properties.len() as u32;
        let count = props.len() as u32;
        self.properties.extend_from_slice(props);
        let node = self.get_node_mut(node_id);
        node.props_start = start;
        node.props_count = count;
    }

    pub fn get_properties(&self, id: u32) -> &[Property] {
        let node = self.get_node(id);
        &self.properties[node.props_start as usize..(node.props_start + node.props_count) as usize]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for HastArena {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HastBuilder {
    arena: HastArena,
    stack: Vec<(u32, Vec<u32>)>, // (node_id, children_so_far)
}

impl HastBuilder {
    pub fn new() -> Self {
        Self {
            arena: HastArena::new(),
            stack: Vec::new(),
        }
    }

    pub fn arena(&self) -> &HastArena {
        &self.arena
    }

    pub fn alloc_string(&mut self, s: &str) -> StringRef {
        self.arena.alloc_string(s)
    }

    pub fn open_element(&mut self, tag: &str) -> u32 {
        let id = self.arena.alloc_node(HastNodeType::Element);
        let tag_ref = self.arena.alloc_string(tag);
        self.arena.get_node_mut(id).tag_name = tag_ref;
        self.stack.push((id, Vec::new()));
        id
    }

    pub fn open_root(&mut self) -> u32 {
        let id = self.arena.alloc_node(HastNodeType::Root);
        self.stack.push((id, Vec::new()));
        id
    }

    pub fn add_text(&mut self, text: &str) -> u32 {
        let id = self.arena.alloc_node(HastNodeType::Text);
        let text_ref = self.arena.alloc_string(text);
        self.arena.get_node_mut(id).value = text_ref;
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(id);
        }
        id
    }

    pub fn add_raw(&mut self, html: &str) -> u32 {
        let id = self.arena.alloc_node(HastNodeType::Raw);
        let html_ref = self.arena.alloc_string(html);
        self.arena.get_node_mut(id).value = html_ref;
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(id);
        }
        id
    }

    pub fn add_comment(&mut self, text: &str) -> u32 {
        let id = self.arena.alloc_node(HastNodeType::Comment);
        let text_ref = self.arena.alloc_string(text);
        self.arena.get_node_mut(id).value = text_ref;
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(id);
        }
        id
    }

    pub fn add_doctype(&mut self) -> u32 {
        let id = self.arena.alloc_node(HastNodeType::Doctype);
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(id);
        }
        id
    }

    pub fn set_properties(&mut self, node_id: u32, props: &[Property]) {
        self.arena.add_properties(node_id, props);
    }

    /// Open an MDX JSX element node (flow or text).
    pub fn open_mdx_jsx_element(&mut self, node_type: HastNodeType, name: &str) -> u32 {
        let id = self.arena.alloc_node(node_type);
        if !name.is_empty() {
            let tag_ref = self.arena.alloc_string(name);
            self.arena.get_node_mut(id).tag_name = tag_ref;
        }
        self.stack.push((id, Vec::new()));
        id
    }

    /// Add an MDX value leaf node (expression or ESM).
    pub fn add_mdx_value_node(&mut self, node_type: HastNodeType, value: &str) -> u32 {
        let id = self.arena.alloc_node(node_type);
        let value_ref = self.arena.alloc_string(value);
        self.arena.get_node_mut(id).value = value_ref;
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(id);
        }
        id
    }

    pub fn set_position(&mut self, node_id: u32, start_line: u32, end_line: u32) {
        let node = self.arena.get_node_mut(node_id);
        node.start_line = start_line;
        node.end_line = end_line;
    }

    /// Close the current element, attaching it to its parent.
    pub fn close(&mut self) -> u32 {
        let (node_id, children) = self.stack.pop().expect("close called with empty stack");
        self.arena.set_children(node_id, &children);
        if let Some((_, parent_children)) = self.stack.last_mut() {
            parent_children.push(node_id);
        }
        node_id
    }

    pub fn finish(mut self) -> HastArena {
        // Close any remaining open nodes (except the root at position 0)
        while self.stack.len() > 1 {
            self.close();
        }
        if let Some((node_id, children)) = self.stack.pop() {
            self.arena.set_children(node_id, &children);
        }
        self.arena
    }
}

impl Default for HastBuilder {
    fn default() -> Self {
        Self::new()
    }
}
