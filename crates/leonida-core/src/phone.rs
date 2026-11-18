//! LeonidaOS mobile companion (Phone) data models and helpers (v0.6).

use serde::{Deserialize, Serialize};

/// Informant or companion contact entry in LeonidaOS Messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneContact {
    pub id: String,
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub district: String,
    #[serde(default = "default_avatar_color")]
    pub avatar_color: String,
    #[serde(default)]
    pub bio: String,
}

fn default_avatar_color() -> String {
    "#38bdf8".to_string()
}

/// Chat message linked to a [`PhoneContact`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneMessage {
    pub id: String,
    pub contact_id: String,
    pub sender: String,
    pub text: String,
    pub timestamp: i64,
    #[serde(default)]
    pub unread: bool,
}

/// Geotagged Snapmatic photo with optional filter and caption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapmaticPhoto {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub district: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub image_data_url: String,
    pub created_at: i64,
}

/// Field note optionally pinned to a Leonida world coordinate `(x, y)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneNote {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    pub updated_at: i64,
}

/// Aggregate counts across Phone companion data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneStats {
    pub contacts: usize,
    pub messages: usize,
    pub unread_messages: usize,
    pub photos: usize,
    pub notes: usize,
}

/// Compute aggregate Phone statistics without panicking.
pub fn phone_stats(
    contacts: &[PhoneContact],
    messages: &[PhoneMessage],
    photos: &[SnapmaticPhoto],
    notes: &[PhoneNote],
) -> PhoneStats {
    PhoneStats {
        contacts: contacts.len(),
        messages: messages.len(),
        unread_messages: messages.iter().filter(|m| m.unread).count(),
        photos: photos.len(),
        notes: notes.len(),
    }
}

/// Return messages for `contact_id` sorted chronologically by `timestamp`.
pub fn messages_for_contact(messages: &[PhoneMessage], contact_id: &str) -> Vec<PhoneMessage> {
    let mut thread: Vec<PhoneMessage> = messages
        .iter()
        .filter(|m| m.contact_id == contact_id)
        .cloned()
        .collect();
    thread.sort_by_key(|m| m.timestamp);
    thread
}

/// Count unread messages in a slice.
pub fn unread_count(messages: &[PhoneMessage]) -> usize {
    messages.iter().filter(|m| m.unread).count()
}

/// Filter Snapmatic photos by optional district name (case-insensitive).
pub fn filter_photos_by_district(
    photos: &[SnapmaticPhoto],
    district: Option<&str>,
) -> Vec<SnapmaticPhoto> {
    match district.map(str::trim).filter(|d| !d.is_empty()) {
        None => photos.to_vec(),
        Some(target) => {
            let needle = target.to_lowercase();
            photos
                .iter()
                .filter(|p| p.district.to_lowercase() == needle)
                .cloned()
                .collect()
        }
    }
}

/// Built-in LeonidaOS starter contacts.
pub fn default_phone_contacts() -> Vec<PhoneContact> {
    vec![
        PhoneContact {
            id: "lucia".to_string(),
            name: "Lucia".to_string(),
            role: "Partner".to_string(),
            district: "Vice Beach".to_string(),
            avatar_color: "#f43f5e".to_string(),
            bio: "Coordinating Vice Beach & Ocean View leads.".to_string(),
        },
        PhoneContact {
            id: "jason".to_string(),
            name: "Jason".to_string(),
            role: "Wheelman".to_string(),
            district: "Port Vice".to_string(),
            avatar_color: "#38bdf8".to_string(),
            bio: "Tracking routes, safehouses, and Port Vice activity.".to_string(),
        },
        PhoneContact {
            id: "boobie".to_string(),
            name: "Boobie Ike".to_string(),
            role: "Fixer".to_string(),
            district: "Downtown Vice".to_string(),
            avatar_color: "#f59e0b".to_string(),
            bio: "Downtown real estate & club intelligence.".to_string(),
        },
    ]
}
