use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

/// Type of media attached to a story.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StoryMediaType {
    Image,
    Video,
}

/// A single WhatsApp Status / Story item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryItem {
    pub id: String,
    pub sender: String,
    pub sender_name: Option<String>,
    pub timestamp: u64,
    pub text: Option<String>,
    pub background_argb: Option<u32>,
    pub font: Option<u32>,
    pub media_type: Option<StoryMediaType>,
    pub caption: Option<String>,
    pub thumbnail: Option<Vec<u8>>,
    pub viewed: bool,
}

/// A contact and all their active (within 24 hours) stories.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContactStories {
    pub sender: String,
    pub sender_name: String,
    pub items: Vec<StoryItem>,
    pub has_unviewed: bool,
    pub latest_timestamp: u64,
}

/// Store for managing all cached and live stories.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoriesStore {
    pub stories_by_sender: HashMap<String, Vec<StoryItem>>,
}

impl StoriesStore {
    /// Loads stories from disk, returning an empty store if absent or corrupt.
    pub fn load(path: &Path) -> Self {
        let Ok(data) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    /// Persists stories to disk.
    pub fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string(self) {
            let _ = fs::write(path, json);
        }
    }

    /// Adds a story item, ignoring duplicate IDs.
    pub fn add(&mut self, item: StoryItem) {
        let sender = item.sender.clone();
        let list = self.stories_by_sender.entry(sender).or_default();
        if !list.iter().any(|existing| existing.id == item.id) {
            list.push(item);
            list.sort_by_key(|s| s.timestamp);
        }
    }

    /// Marks a specific story item as viewed.
    pub fn mark_viewed(&mut self, sender: &str, id: &str) {
        if let Some(list) = self.stories_by_sender.get_mut(sender) {
            for item in list.iter_mut() {
                if item.id == id {
                    item.viewed = true;
                }
            }
        }
    }

    /// Removes stories older than 24 hours (86,400 seconds).
    pub fn clean_expired(&mut self, now_secs: u64) {
        let cutoff = now_secs.saturating_sub(24 * 3600);
        self.stories_by_sender.retain(|_, items| {
            items.retain(|item| item.timestamp >= cutoff);
            !items.is_empty()
        });
    }

    /// Returns contacts grouped with their stories, sorted:
    /// Contacts with unviewed stories come first, then sorted by latest timestamp descending.
    pub fn grouped(&self) -> Vec<ContactStories> {
        let mut groups: Vec<ContactStories> = self
            .stories_by_sender
            .iter()
            .filter(|(_, items)| !items.is_empty())
            .map(|(sender, items)| {
                let latest_timestamp = items.last().map(|s| s.timestamp).unwrap_or(0);
                let sender_name = items
                    .iter()
                    .rev()
                    .find_map(|s| s.sender_name.clone())
                    .unwrap_or_else(|| sender.clone());
                let has_unviewed = items.iter().any(|s| !s.viewed);
                ContactStories {
                    sender: sender.clone(),
                    sender_name,
                    items: items.clone(),
                    has_unviewed,
                    latest_timestamp,
                }
            })
            .collect();

        groups.sort_by(|a, b| {
            match (a.has_unviewed, b.has_unviewed) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.latest_timestamp.cmp(&a.latest_timestamp),
            }
        });

        groups
    }

    /// Returns (next_sender, next_index) if available.
    pub fn next_item(&self, current_sender: &str, current_idx: usize) -> Option<(String, usize)> {
        let groups = self.grouped();
        let group_idx = groups.iter().position(|g| g.sender == current_sender)?;
        let group = &groups[group_idx];
        if current_idx + 1 < group.items.len() {
            Some((current_sender.to_owned(), current_idx + 1))
        } else if group_idx + 1 < groups.len() {
            Some((groups[group_idx + 1].sender.clone(), 0))
        } else {
            None
        }
    }

    /// Returns (prev_sender, prev_index) if available.
    pub fn prev_item(&self, current_sender: &str, current_idx: usize) -> Option<(String, usize)> {
        let groups = self.grouped();
        let group_idx = groups.iter().position(|g| g.sender == current_sender)?;
        if current_idx > 0 {
            Some((current_sender.to_owned(), current_idx - 1))
        } else if group_idx > 0 {
            let prev_group = &groups[group_idx - 1];
            let last_idx = prev_group.items.len().saturating_sub(1);
            Some((prev_group.sender.clone(), last_idx))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stories_store_add_and_grouped() {
        let mut store = StoriesStore::default();
        store.add(StoryItem {
            id: "msg-1".into(),
            sender: "alice@s.whatsapp.net".into(),
            sender_name: Some("Alice".into()),
            timestamp: 1000,
            text: Some("Hello world".into()),
            background_argb: Some(0xFF00A884),
            font: Some(0),
            media_type: None,
            caption: None,
            thumbnail: None,
            viewed: false,
        });

        store.add(StoryItem {
            id: "msg-2".into(),
            sender: "bob@s.whatsapp.net".into(),
            sender_name: Some("Bob".into()),
            timestamp: 1050,
            text: Some("Bob's status".into()),
            background_argb: None,
            font: None,
            media_type: None,
            caption: None,
            thumbnail: None,
            viewed: false,
        });

        let groups = store.grouped();
        assert_eq!(groups.len(), 2);

        // Bob posted later than Alice, both unviewed, so Bob comes first
        assert_eq!(groups[0].sender, "bob@s.whatsapp.net");
        assert_eq!(groups[1].sender, "alice@s.whatsapp.net");

        // Mark Bob's status as viewed
        store.mark_viewed("bob@s.whatsapp.net", "msg-2");
        let groups = store.grouped();
        // Alice has unviewed, Bob is viewed -> Alice comes first now
        assert_eq!(groups[0].sender, "alice@s.whatsapp.net");
        assert_eq!(groups[1].sender, "bob@s.whatsapp.net");
    }

    #[test]
    fn test_stories_store_navigation() {
        let mut store = StoriesStore::default();
        store.add(StoryItem {
            id: "a1".into(),
            sender: "alice".into(),
            sender_name: None,
            timestamp: 100,
            text: Some("A1".into()),
            background_argb: None,
            font: None,
            media_type: None,
            caption: None,
            thumbnail: None,
            viewed: false,
        });
        store.add(StoryItem {
            id: "a2".into(),
            sender: "alice".into(),
            sender_name: None,
            timestamp: 101,
            text: Some("A2".into()),
            background_argb: None,
            font: None,
            media_type: None,
            caption: None,
            thumbnail: None,
            viewed: false,
        });
        store.add(StoryItem {
            id: "b1".into(),
            sender: "bob".into(),
            sender_name: None,
            timestamp: 90,
            text: Some("B1".into()),
            background_argb: None,
            font: None,
            media_type: None,
            caption: None,
            thumbnail: None,
            viewed: false,
        });

        // Next item within same sender
        assert_eq!(store.next_item("alice", 0), Some(("alice".to_string(), 1)));
        // Next item advances to next sender
        assert_eq!(store.next_item("alice", 1), Some(("bob".to_string(), 0)));
        // Next item on last sender is None
        assert_eq!(store.next_item("bob", 0), None);

        // Prev item
        assert_eq!(store.prev_item("bob", 0), Some(("alice".to_string(), 1)));
        assert_eq!(store.prev_item("alice", 1), Some(("alice".to_string(), 0)));
        assert_eq!(store.prev_item("alice", 0), None);
    }
}

