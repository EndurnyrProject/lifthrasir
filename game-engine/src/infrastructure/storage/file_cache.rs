use std::collections::HashMap;

pub struct FileCache {
    cache: HashMap<String, Vec<u8>>,
    max_size: usize,
    current_size: usize,
}

impl FileCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_size,
            current_size: 0,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.cache.get(key)
    }

    pub fn insert(&mut self, key: String, value: Vec<u8>) {
        if self.current_size + value.len() > self.max_size {
            self.evict_lru();
        }

        self.current_size += value.len();
        self.cache.insert(key, value);
    }

    fn evict_lru(&mut self) {
        // Simple eviction: remove first entry
        if let Some((key, _)) = self.cache.iter().next() {
            let key = key.clone();
            if let Some(value) = self.cache.remove(&key) {
                self.current_size -= value.len();
            }
        }
    }
}
