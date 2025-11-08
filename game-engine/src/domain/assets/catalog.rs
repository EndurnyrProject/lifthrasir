use std::collections::HashMap;

/// Generic catalog for any asset type with O(1) lookup
pub struct AssetCatalog<T> {
    items: Vec<T>,
    by_id: HashMap<u16, usize>,
}

impl<T> AssetCatalog<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            by_id: HashMap::new(),
        }
    }

    /// Add an item to the catalog with its ID
    pub fn add(&mut self, id: u16, item: T) {
        let index = self.items.len();
        self.items.push(item);
        self.by_id.insert(id, index);
    }

    /// Get an item by ID
    pub fn get(&self, id: u16) -> Option<&T> {
        self.by_id.get(&id).and_then(|&idx| self.items.get(idx))
    }

    /// Iterate over all items
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Get the number of items in the catalog
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the catalog is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<T> Default for AssetCatalog<T> {
    fn default() -> Self {
        Self::new()
    }
}
