//! Compact records with optional line/column storage. The 52-byte wire layout is unchanged.
use crate::ArenaNode;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Record {
    pub(crate) kind: u32,
    pub(crate) parent: u32,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) children_start: u32,
    pub(crate) children_count: u32,
    pub(crate) data_offset: u32,
    pub(crate) data_len: u32,
}
const _: () = assert!(size_of::<Record>() == 32);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeStore {
    records: Vec<Record>,
    positions: Vec<[u32; 4]>,
}
impl NodeStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: Vec::with_capacity(capacity),
            positions: Vec::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.records.capacity()
    }
    pub fn storage_bytes(&self) -> usize {
        self.records.capacity() * size_of::<Record>()
            + self.positions.capacity() * size_of::<[u32; 4]>()
    }
    pub fn reserve(&mut self, additional: usize) {
        self.records.reserve(additional);
    }
    pub fn clear(&mut self) {
        self.records.clear();
        self.positions.clear();
    }
    #[inline(always)]
    pub fn children_count(&self, id: usize) -> u32 {
        self.records[id].children_count
    }
    #[inline(always)]
    pub fn get(&self, id: usize) -> ArenaNode {
        let r = self.records[id];
        let p = if self.positions.is_empty() {
            [0; 4]
        } else {
            // SAFETY: a nonempty positions vector has exactly one entry per record.
            // The checked record access above proves this index is in range.
            unsafe { *self.positions.get_unchecked(id) }
        };
        ArenaNode {
            id: id as u32,
            node_type: r.kind as u8,
            _pad: [0; 3],
            parent: r.parent,
            start_offset: r.start,
            end_offset: r.end,
            start_line: p[0],
            start_column: p[1],
            end_line: p[2],
            end_column: p[3],
            children_start: r.children_start,
            children_count: r.children_count,
            data_offset: r.data_offset,
            data_len: r.data_len,
        }
    }
    #[inline(always)]
    fn record(n: &ArenaNode) -> Record {
        Record {
            kind: n.node_type as u32,
            parent: n.parent,
            start: n.start_offset,
            end: n.end_offset,
            children_start: n.children_start,
            children_count: n.children_count,
            data_offset: n.data_offset,
            data_len: n.data_len,
        }
    }
    #[inline(always)]
    fn set_node_lines(&mut self, id: usize, n: &ArenaNode) {
        self.set_lines(id, [n.start_line, n.start_column, n.end_line, n.end_column]);
    }
    #[inline(always)]
    pub(crate) fn set_lines(&mut self, id: usize, position: [u32; 4]) {
        if self.positions.is_empty() {
            if position == [0; 4] {
                return;
            }
            self.initialize_positions();
        }
        self.positions[id] = position;
    }
    #[inline(always)]
    fn push_lines(&mut self, id: usize, position: [u32; 4]) {
        if self.positions.is_empty() {
            self.set_lines(id, position);
        } else {
            self.positions.push(position);
        }
    }
    #[cold]
    #[inline(never)]
    fn initialize_positions(&mut self) {
        self.positions.reserve(self.records.capacity());
        self.positions.resize(self.records.len(), [0; 4]);
    }
    #[inline(always)]
    pub fn push_leaf(
        &mut self,
        node_type: u8,
        parent: u32,
        position: crate::NodePosition,
        data_offset: u32,
        data_len: u32,
    ) -> u32 {
        let id = self.records.len();
        self.records.push(Record {
            kind: node_type as u32,
            parent,
            start: position.start_offset,
            end: position.end_offset,
            children_start: 0,
            children_count: 0,
            data_offset,
            data_len,
        });
        self.push_lines(
            id,
            [
                position.start_line,
                position.start_column,
                position.end_line,
                position.end_column,
            ],
        );
        id as u32
    }
    #[inline(always)]
    pub fn set_parent(&mut self, id: usize, parent: u32) {
        self.records[id].parent = parent;
    }
    #[inline(always)]
    pub fn set_children_range(&mut self, id: usize, start: u32, count: u32) {
        self.records[id].children_start = start;
        self.records[id].children_count = count;
    }
    #[inline(always)]
    pub fn set_data_range(&mut self, id: usize, offset: u32, len: u32) {
        self.records[id].data_offset = offset;
        self.records[id].data_len = len;
    }
    #[inline(always)]
    pub fn set_position(&mut self, id: usize, p: crate::NodePosition) {
        self.records[id].start = p.start_offset;
        self.records[id].end = p.end_offset;
        self.set_lines(id, [p.start_line, p.start_column, p.end_line, p.end_column]);
    }
    #[inline(always)]
    pub fn push(&mut self, node: ArenaNode) {
        let id = self.records.len();
        assert_eq!(node.id as usize, id, "node IDs must equal record indices");
        self.records.push(Self::record(&node));
        self.push_lines(
            id,
            [
                node.start_line,
                node.start_column,
                node.end_line,
                node.end_column,
            ],
        );
    }
    #[inline(always)]
    pub fn edit(&mut self, id: usize) -> NodeEdit<'_> {
        NodeEdit {
            value: self.get(id),
            store: self,
            id,
        }
    }
    pub fn iter(&self) -> NodeIter<'_> {
        NodeIter {
            store: self,
            next: 0,
        }
    }
}

pub struct NodeEdit<'a> {
    value: ArenaNode,
    store: &'a mut NodeStore,
    id: usize,
}
impl Deref for NodeEdit<'_> {
    type Target = ArenaNode;
    #[inline(always)]
    fn deref(&self) -> &ArenaNode {
        &self.value
    }
}
impl DerefMut for NodeEdit<'_> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut ArenaNode {
        &mut self.value
    }
}
impl Drop for NodeEdit<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        assert_eq!(self.value.id as usize, self.id, "node IDs are immutable");
        self.store.records[self.id] = NodeStore::record(&self.value);
        self.store.set_node_lines(self.id, &self.value);
    }
}

pub struct NodeIter<'a> {
    store: &'a NodeStore,
    next: usize,
}
impl Iterator for NodeIter<'_> {
    type Item = ArenaNode;
    fn next(&mut self) -> Option<ArenaNode> {
        if self.next == self.store.len() {
            None
        } else {
            let n = self.store.get(self.next);
            self.next += 1;
            Some(n)
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.store.len() - self.next;
        (n, Some(n))
    }
}
impl ExactSizeIterator for NodeIter<'_> {}
impl<'a> IntoIterator for &'a NodeStore {
    type Item = ArenaNode;
    type IntoIter = NodeIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_are_optional_and_edits_round_trip() {
        let mut s = NodeStore::new();
        for i in 0..10 {
            s.push(ArenaNode::new(i, 2));
        }
        assert!(s.positions.is_empty());
        {
            let mut n = s.edit(4);
            n.start_line = 2;
            n.end_column = 9;
            n.parent = 3;
        }
        assert_eq!(s.get(4).start_line, 2);
        assert_eq!(s.get(4).end_column, 9);
        assert_eq!(s.get(4).parent, 3);
        assert_eq!(s.get(3).start_line, 0);
        s.push(ArenaNode::new(10, 2));
        assert_eq!(s.get(10).end_column, 0);
        s.clear();
        s.push(ArenaNode::new(0, 0));
        assert!(s.positions.is_empty());
        assert_eq!(s.get(0).start_line, 0);
    }
}
