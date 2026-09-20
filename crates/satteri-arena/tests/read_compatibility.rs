use satteri_arena::{Arena, ArenaNode, ArenaRead, Mdast};
use std::sync::{Arc, Mutex};

#[test]
fn owned_storage_and_reference_accessors_remain_available() {
    let mut arena = Arena::<Mdast>::new("source".into());
    let nodes: &mut Vec<ArenaNode> = &mut arena.nodes;
    nodes.push(ArenaNode::new(0, 1));
    let node: &mut ArenaNode = arena.get_node_mut(0);
    node.end_offset = 6;
    let node: &ArenaNode = arena.get_node(0);
    assert_eq!(node.end_offset, 6);
    assert_eq!(ArenaRead::get_node(&arena, 0).end_offset, 6);
}

#[test]
fn read_views_accept_existing_deref_wrappers() {
    fn end(view: &impl ArenaRead<Mdast>) -> u32 {
        view.get_node(0).end_offset
    }
    let mut arena = Arena::<Mdast>::new("source".into());
    arena.alloc_node(1);
    arena.get_node_mut(0).end_offset = 6;
    let boxed = Box::new(arena);
    assert_eq!(end(&boxed), 6);
    let shared = Arc::new(boxed);
    assert_eq!(end(&shared), 6);
    let locked = Mutex::new(shared);
    assert_eq!(end(&locked.lock().unwrap()), 6);
}
