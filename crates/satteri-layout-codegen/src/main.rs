//! Standalone layout/node-type code generator.
//!
//! Reads the node registry in [`schema`] and writes the generated Rust and TS
//! into the `generated/` folders. Run via `pnpm codegen` (or directly with
//! `cargo run -p satteri-layout-codegen`); the output is committed to git and a
//! CI check reruns this and fails on any diff.

mod emit;
mod schema;

use std::fs;
use std::path::{Path, PathBuf};

use schema::{HAST_NODES, HAST_STRUCTS, MDAST_NODES, MDAST_STRUCTS};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve workspace root");

    let mdast_rs = root.join("crates/satteri-ast/src/mdast/generated");
    let hast_rs = root.join("crates/satteri-ast/src/hast/generated");
    let ast_rs = root.join("crates/satteri-ast/src/generated");
    let arena_rs = root.join("crates/satteri-arena/src/generated");
    let plugin_rs = root.join("crates/satteri-plugin-api/src/generated");
    let mdast_ts = root.join("packages/satteri/src/mdast/generated");
    let hast_ts = root.join("packages/satteri/src/hast/generated");
    let shared_ts = root.join("packages/satteri/src/generated");
    let dirs = [
        &mdast_rs, &hast_rs, &ast_rs, &arena_rs, &plugin_rs, &mdast_ts, &hast_ts, &shared_ts,
    ];
    for dir in dirs {
        fs::create_dir_all(dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    }

    // Fail generation outright if the struct table and the field lists drifted.
    schema::check_struct_layouts();

    let mdast_layouts = schema::layouts(MDAST_NODES);
    let mdast_tails = schema::tail_layouts(MDAST_NODES);
    let hast_layouts = schema::layouts(HAST_NODES);
    let hast_tails = schema::tail_layouts(HAST_NODES);

    let mut written: Vec<PathBuf> = Vec::new();
    let mut write = |path: &Path, contents: &str| {
        written.push(path.to_path_buf());
        write_atomic(path, contents);
    };

    // MDAST (Rust)
    write(&mdast_rs.join("mod.rs"), MDAST_MOD_RS);
    write(
        &mdast_rs.join("node_types.rs"),
        &emit::node_types_rs("MdastNodeType", MDAST_NODES),
    );
    write(
        &mdast_rs.join("walk_type_data.rs"),
        &emit::walk_rs(
            "write_mdast_type_data_inline",
            "Mdast",
            &mdast_layouts,
            &mdast_tails,
        ),
    );
    write(
        &mdast_rs.join("assert_layouts.rs"),
        &emit::asserts_rs("crate::mdast::codec", MDAST_STRUCTS),
    );

    // HAST (Rust)
    write(&hast_rs.join("mod.rs"), HAST_MOD_RS);
    write(
        &hast_rs.join("node_types.rs"),
        &emit::node_types_rs("HastNodeType", HAST_NODES),
    );
    write(
        &hast_rs.join("walk_type_data.rs"),
        &emit::walk_rs(
            "write_hast_type_data_inline",
            "Hast",
            &hast_layouts,
            &hast_tails,
        ),
    );
    write(
        &hast_rs.join("assert_layouts.rs"),
        &emit::asserts_rs("crate::hast::codec", HAST_STRUCTS),
    );

    // Shared wire constants (satteri-ast, re-exported by shared.rs)
    write(&ast_rs.join("mod.rs"), AST_MOD_RS);
    write(
        &ast_rs.join("wire_constants.rs"),
        &emit::wire_constants_rs(schema::AST_WIRE_TABLES, AST_WC_DOC),
    );

    // Arena raw-buffer layout (header offsets + struct pins)
    write(&arena_rs.join("mod.rs"), ARENA_MOD_RS);
    write(&arena_rs.join("layout.rs"), &emit::arena_layout_rs());

    // Plugin-api encoders + wire constants
    write(&plugin_rs.join("mod.rs"), PLUGIN_MOD_RS);
    write(
        &plugin_rs.join("encode.rs"),
        &emit::encode_rs(&mdast_layouts, &mdast_tails, &hast_tails),
    );
    write(
        &plugin_rs.join("prop_slots.rs"),
        &emit::prop_slots_rs(MDAST_NODES),
    );
    write(
        &plugin_rs.join("wire_constants.rs"),
        &emit::wire_constants_rs(schema::PLUGIN_WIRE_TABLES, PLUGIN_WC_DOC),
    );

    // MDAST (TS)
    write(
        &mdast_ts.join("node-types.ts"),
        &emit::node_types_ts(MDAST_NODES, "MDAST", schema::MDAST_OPSTREAM_EXCLUDED, true),
    );
    write(
        &mdast_ts.join("layout.ts"),
        &emit::layout_ts(&mdast_layouts, &mdast_tails),
    );

    // HAST (TS)
    write(
        &hast_ts.join("node-types.ts"),
        &emit::node_types_ts(HAST_NODES, "HAST", schema::HAST_OPSTREAM_EXCLUDED, false),
    );

    // Tree-agnostic TS (wire constants + arena layout)
    write(
        &shared_ts.join("wire-constants.ts"),
        &emit::wire_constants_ts(schema::TS_WIRE_TABLES),
    );
    write(&shared_ts.join("arena-layout.ts"), &emit::arena_layout_ts());

    // The generated dirs hold nothing but this generator's output: remove any
    // file it no longer emits, so a retired output can't linger committed
    // (the CI diff check then flags the deletion).
    for dir in dirs {
        let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
                .path();
            if path.is_file() && !written.contains(&path) {
                fs::remove_file(&path).unwrap_or_else(|e| panic!("remove {}: {e}", path.display()));
                eprintln!("removed {}", path.display());
            }
        }
    }
}

const MDAST_MOD_RS: &str = "//! @generated by `cargo run -p satteri-layout-codegen`. Do not edit by hand.\n\n\
    pub mod node_types;\npub(crate) mod walk_type_data;\nmod assert_layouts;\n";

const HAST_MOD_RS: &str = "//! @generated by `cargo run -p satteri-layout-codegen`. Do not edit by hand.\n\n\
    pub mod node_types;\npub(crate) mod walk_type_data;\nmod assert_layouts;\n";

const PLUGIN_MOD_RS: &str = "//! @generated by `cargo run -p satteri-layout-codegen`. Do not edit by hand.\n\n\
    pub(crate) mod encode;\npub(crate) mod prop_slots;\npub(crate) mod wire_constants;\n";

const AST_MOD_RS: &str = "//! @generated by `cargo run -p satteri-layout-codegen`. Do not edit by hand.\n\n\
    pub mod wire_constants;\n";

const ARENA_MOD_RS: &str = "//! @generated by `cargo run -p satteri-layout-codegen`. Do not edit by hand.\n\n\
    pub(crate) mod layout;\n";

const PLUGIN_WC_DOC: &[&str] = &[
    "JS<->Rust wire-protocol byte values for the command buffer and the",
    "op-stream replay. Declared once in `satteri-layout-codegen/src/schema.rs`;",
    "the JS twin is `packages/satteri/src/generated/wire-constants.ts`.",
];

const AST_WC_DOC: &[&str] = &[
    "Property and MDX-attribute kind bytes shared by codecs and the command",
    "wire. Declared once in `satteri-layout-codegen/src/schema.rs`; re-exported",
    "by `shared.rs`, with the JS twin in",
    "`packages/satteri/src/generated/wire-constants.ts`.",
];

/// Write only when the content changed, so an unchanged rerun touches nothing
/// (and reports cleanly in the CI diff check). Changed content lands via a
/// sibling temp file + rename, so an interrupted run can't leave a
/// half-written generated file behind.
fn write_atomic(path: &Path, contents: &str) {
    let changed = fs::read_to_string(path)
        .map(|old| old != contents)
        .unwrap_or(true);
    if !changed {
        eprintln!("unchanged {}", path.display());
        return;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents).unwrap_or_else(|e| panic!("write {}: {e}", tmp.display()));
    fs::rename(&tmp, path).unwrap_or_else(|e| panic!("rename {}: {e}", path.display()));
    eprintln!("wrote {}", path.display());
}
