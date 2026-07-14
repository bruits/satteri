use satteri_arena::{Arena, ArenaBuilder, Hast, StringRef};
use satteri_ast::hast::codec::encode_element_data;
use satteri_ast::hast::HastNodeType;
use satteri_ast::patch::{Patch, PatchContent};

/// One keep-children `<span class="link">` Replace per `<a>` element (the link-transform shape).
pub fn link_replace_patches(hast: &Arena<Hast>) -> Vec<Patch<Hast>> {
    let mut patches = Vec::new();
    for id in 0..hast.len() as u32 {
        let node = hast.get_node(id);
        if node.node_type != HastNodeType::Element as u8 {
            continue;
        }
        let td = hast.get_type_data(id);
        if td.len() < 8 || hast.get_str(StringRef::from_bytes(&td[0..8])) != "a" {
            continue;
        }
        let mut b = ArenaBuilder::<Hast>::new(String::new());
        let tag = b.alloc_string("span");
        let name = b.alloc_string("className");
        let val = b.alloc_string("link");
        b.open_node(HastNodeType::Element as u8);
        b.set_data_current(&encode_element_data(tag, &[(name, 0, val)]));
        b.close_node();
        patches.push(Patch::Replace {
            node_id: id,
            new_tree: PatchContent::Tree(b.finish()),
            keep_children: true,
        });
    }
    patches
}
