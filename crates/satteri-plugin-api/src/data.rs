use rustc_hash::FxHashMap;
use std::any::{Any, TypeId};

/// A value that can be stored in the untyped data map (interoperable with JS node.data).
#[derive(Debug, Clone)]
pub enum DataValue {
    String(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    Null,
}

impl DataValue {
    pub fn as_str(&self) -> Option<&str> {
        if let DataValue::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let DataValue::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn as_int(&self) -> Option<i64> {
        if let DataValue::Int(i) = self {
            Some(*i)
        } else {
            None
        }
    }
}

/// Untyped data map: maps node_id → (key → DataValue).
/// This is the Rust-side of the JS node.data map.
/// When a JS plugin runs after a Rust plugin, this gets synced to the JS DataMap.
///
/// Nested layout lets `get`/`has`/`remove` look up by `&str` without allocating;
/// `set` only allocates when inserting a key for the first time.
#[derive(Debug, Default)]
pub struct DataMap {
    inner: FxHashMap<u32, FxHashMap<String, DataValue>>,
}

impl DataMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, node_id: u32, key: &str, value: DataValue) {
        let bucket = self.inner.entry(node_id).or_default();
        if let Some(slot) = bucket.get_mut(key) {
            *slot = value;
        } else {
            bucket.insert(key.to_string(), value);
        }
    }

    pub fn get(&self, node_id: u32, key: &str) -> Option<&DataValue> {
        self.inner.get(&node_id)?.get(key)
    }

    pub fn remove(&mut self, node_id: u32, key: &str) {
        if let Some(bucket) = self.inner.get_mut(&node_id) {
            bucket.remove(key);
        }
    }

    pub fn has(&self, node_id: u32, key: &str) -> bool {
        self.inner
            .get(&node_id)
            .is_some_and(|bucket| bucket.contains_key(key))
    }

    /// Iterate all entries for a given node_id
    pub fn entries_for_node(&self, node_id: u32) -> impl Iterator<Item = (&str, &DataValue)> {
        self.inner
            .get(&node_id)
            .into_iter()
            .flat_map(|bucket| bucket.iter().map(|(key, val)| (key.as_str(), val)))
    }

    pub fn len(&self) -> usize {
        self.inner.values().map(FxHashMap::len).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.values().all(FxHashMap::is_empty)
    }
}

/// Typed data map: stores strongly-typed data keyed by TypeId + node_id.
/// Rust-only, never crosses to JS.
pub struct TypedDataMap {
    inner: FxHashMap<(u32, TypeId), Box<dyn Any + Send + Sync>>,
}

impl TypedDataMap {
    pub fn new() -> Self {
        Self {
            inner: FxHashMap::default(),
        }
    }

    pub fn set<T: Any + Send + Sync>(&mut self, node_id: u32, value: T) {
        self.inner
            .insert((node_id, TypeId::of::<T>()), Box::new(value));
    }

    pub fn get<T: Any + Send + Sync>(&self, node_id: u32) -> Option<&T> {
        self.inner
            .get(&(node_id, TypeId::of::<T>()))
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    pub fn remove<T: Any + Send + Sync>(&mut self, node_id: u32) {
        self.inner.remove(&(node_id, TypeId::of::<T>()));
    }

    pub fn has<T: Any + Send + Sync>(&self, node_id: u32) -> bool {
        self.inner.contains_key(&(node_id, TypeId::of::<T>()))
    }
}

impl Default for TypedDataMap {
    fn default() -> Self {
        Self::new()
    }
}
