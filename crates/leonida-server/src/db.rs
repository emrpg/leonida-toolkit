use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MARKERS_SCHEMA: &str = "leonida-user-markers-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserMarker {
    pub id: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkerBundle {
    pub schema: String,
    pub markers: Vec<UserMarker>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMarkerInput {
    pub id: Option<String>,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub note: Option<String>,
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMarkerInput {
    pub name: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub note: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMarkersInput {
    pub schema: Option<String>,
    pub markers: Vec<CreateMarkerInput>,
}

#[derive(Clone)]
pub struct MarkerStore {
    conn: Arc<Mutex<Connection>>,
}

impl MarkerStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create parent dir for {}", path.display()))?;
            }
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open sqlite database {}", path.display()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory sqlite database")?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS user_markers (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 x REAL NOT NULL,
                 y REAL NOT NULL,
                 note TEXT,
                 created_at INTEGER NOT NULL
             );",
        )
        .context("initialize user_markers schema")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn list(&self) -> Result<Vec<UserMarker>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, x, y, note, created_at FROM user_markers ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(UserMarker {
                id: row.get(0)?,
                name: row.get(1)?,
                x: row.get(2)?,
                y: row.get(3)?,
                note: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get(&self, id: &str) -> Result<Option<UserMarker>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.query_row(
            "SELECT id, name, x, y, note, created_at FROM user_markers WHERE id = ?1",
            params![id],
            |row| {
                Ok(UserMarker {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    x: row.get(2)?,
                    y: row.get(3)?,
                    note: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn create(&self, input: CreateMarkerInput) -> Result<UserMarker> {
        let marker = normalize_marker(input);
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO user_markers (id, name, x, y, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 x = excluded.x,
                 y = excluded.y,
                 note = excluded.note,
                 created_at = excluded.created_at",
            params![
                marker.id,
                marker.name,
                marker.x,
                marker.y,
                marker.note,
                marker.created_at
            ],
        )?;
        Ok(marker)
    }

    pub fn update(&self, id: &str, input: UpdateMarkerInput) -> Result<Option<UserMarker>> {
        let Some(mut existing) = self.get(id)? else {
            return Ok(None);
        };
        if let Some(name) = input.name {
            existing.name = name;
        }
        if let Some(x) = input.x {
            existing.x = x;
        }
        if let Some(y) = input.y {
            existing.y = y;
        }
        if let Some(note) = input.note {
            existing.note = note;
        }
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "UPDATE user_markers SET name = ?2, x = ?3, y = ?4, note = ?5 WHERE id = ?1",
            params![
                existing.id,
                existing.name,
                existing.x,
                existing.y,
                existing.note
            ],
        )?;
        Ok(Some(existing))
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let affected = conn.execute("DELETE FROM user_markers WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn clear(&self) -> Result<usize> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let affected = conn.execute("DELETE FROM user_markers", [])?;
        Ok(affected)
    }

    pub fn import(&self, inputs: Vec<CreateMarkerInput>) -> Result<Vec<UserMarker>> {
        let mut conn = self.conn.lock().expect("marker db mutex poisoned");
        let tx = conn.transaction()?;
        let mut imported = Vec::with_capacity(inputs.len());
        {
            let mut stmt = tx.prepare(
                "INSERT INTO user_markers (id, name, x, y, note, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                     name = excluded.name,
                     x = excluded.x,
                     y = excluded.y,
                     note = excluded.note,
                     created_at = excluded.created_at",
            )?;
            for input in inputs {
                let marker = normalize_marker(input);
                stmt.execute(params![
                    marker.id,
                    marker.name,
                    marker.x,
                    marker.y,
                    marker.note,
                    marker.created_at
                ])?;
                imported.push(marker);
            }
        }
        tx.commit()?;
        Ok(imported)
    }
}

fn normalize_marker(input: CreateMarkerInput) -> UserMarker {
    let id = input
        .id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let created_at = input.created_at.unwrap_or_else(now_unix_millis);
    UserMarker {
        id,
        name: input.name,
        x: input.x,
        y: input.y,
        note: input.note,
        created_at,
    }
}

fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
