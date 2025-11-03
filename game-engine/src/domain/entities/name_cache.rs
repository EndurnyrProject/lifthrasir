use bevy::prelude::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Resource)]
pub struct EntityNameCache {
    cache: HashMap<u32, CachedEntityName>,
    request_cooldowns: HashMap<u32, Instant>,
    throttle_duration: Duration,
}

impl Default for EntityNameCache {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityNameCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            request_cooldowns: HashMap::new(),
            throttle_duration: Duration::from_millis(500),
        }
    }

    pub fn get(&self, entity_id: u32) -> Option<&CachedEntityName> {
        self.cache.get(&entity_id).filter(|cached| !cached.is_expired())
    }

    pub fn insert(&mut self, entity_id: u32, name: CachedEntityName) {
        self.cache.insert(entity_id, name);
    }

    pub fn can_request(&self, entity_id: u32) -> bool {
        match self.request_cooldowns.get(&entity_id) {
            None => true,
            Some(last_request) => last_request.elapsed() >= self.throttle_duration,
        }
    }

    pub fn mark_requested(&mut self, entity_id: u32) {
        self.request_cooldowns.insert(entity_id, Instant::now());
    }

    pub fn cleanup_expired(&mut self) {
        self.cache.retain(|_, cached| !cached.is_expired());

        let cutoff = Instant::now() - self.throttle_duration * 10;
        self.request_cooldowns.retain(|_, last_request| *last_request > cutoff);
    }
}

pub struct CachedEntityName {
    pub name: String,
    pub party_name: Option<String>,
    pub guild_name: Option<String>,
    pub position_name: Option<String>,
    cached_at: Instant,
    ttl: Duration,
}

impl CachedEntityName {
    pub fn new(name: String) -> Self {
        Self {
            name,
            party_name: None,
            guild_name: None,
            position_name: None,
            cached_at: Instant::now(),
            ttl: Duration::from_secs(300),
        }
    }

    pub fn with_full_details(
        name: String,
        party_name: String,
        guild_name: String,
        position_name: String,
    ) -> Self {
        Self {
            name,
            party_name: Some(party_name).filter(|s| !s.is_empty()),
            guild_name: Some(guild_name).filter(|s| !s.is_empty()),
            position_name: Some(position_name).filter(|s| !s.is_empty()),
            cached_at: Instant::now(),
            ttl: Duration::from_secs(300),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed() >= self.ttl
    }
}
