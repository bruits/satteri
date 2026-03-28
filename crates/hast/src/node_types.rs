pub const HAST_ROOT: u8 = 0;
pub const HAST_ELEMENT: u8 = 1;
pub const HAST_TEXT: u8 = 2;
pub const HAST_COMMENT: u8 = 3;
pub const HAST_DOCTYPE: u8 = 4;
pub const HAST_RAW: u8 = 5;

// MDX-specific HAST node types
pub const HAST_MDX_JSX_ELEMENT: u8 = 10;
pub const HAST_MDX_JSX_TEXT_ELEMENT: u8 = 11;
pub const HAST_MDX_EXPRESSION: u8 = 12;
pub const HAST_MDX_ESM: u8 = 13;

pub const PROP_STRING: u8 = 0;
pub const PROP_BOOL_TRUE: u8 = 1;
pub const PROP_BOOL_FALSE: u8 = 2;
pub const PROP_SPACE_SEP: u8 = 3;
pub const PROP_COMMA_SEP: u8 = 4;

// MDX JSX attribute kinds (used in HAST_MDX_JSX_ELEMENT type_data)
pub const MDX_ATTR_BOOLEAN_PROP: u8 = 0; // name only, no value
pub const MDX_ATTR_LITERAL_PROP: u8 = 1; // name="literal"
pub const MDX_ATTR_EXPRESSION_PROP: u8 = 2; // name={expr}
pub const MDX_ATTR_SPREAD: u8 = 3; // {...expr}
