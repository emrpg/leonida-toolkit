use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::core_adapter::{CameraObs, CameraPose};

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
             );
             CREATE TABLE IF NOT EXISTS sol_leaderboard (
                 id TEXT PRIMARY KEY,
                 game TEXT NOT NULL,
                 player TEXT NOT NULL,
                 entry_id TEXT,
                 score INTEGER NOT NULL,
                 distance_m REAL NOT NULL,
                 grade TEXT NOT NULL,
                 guess_x REAL NOT NULL,
                 guess_y REAL NOT NULL,
                 target_x REAL NOT NULL,
                 target_y REAL NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_sol_leaderboard_game_score
                 ON sol_leaderboard (game, score DESC, distance_m ASC, created_at ASC);
             CREATE TABLE IF NOT EXISTS sol_notifications (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 body TEXT NOT NULL,
                 read INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS photogrammetry_projects (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 image_name TEXT,
                 width REAL NOT NULL,
                 height REAL NOT NULL,
                 observations_json TEXT NOT NULL,
                 pose_json TEXT,
                 frustum_json TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS companion_progress (
                 id TEXT PRIMARY KEY,
                 completed INTEGER NOT NULL DEFAULT 0,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS phone_contacts (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 role TEXT NOT NULL,
                 district TEXT NOT NULL,
                 avatar_color TEXT NOT NULL,
                 bio TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS phone_messages (
                 id TEXT PRIMARY KEY,
                 contact_id TEXT NOT NULL,
                 sender TEXT NOT NULL,
                 text TEXT NOT NULL,
                 timestamp INTEGER NOT NULL,
                 unread INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS phone_notes (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 body TEXT NOT NULL,
                 x REAL,
                 y REAL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS phone_photos (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 district TEXT NOT NULL,
                 x REAL NOT NULL,
                 y REAL NOT NULL,
                 filter TEXT NOT NULL,
                 caption TEXT NOT NULL,
                 image_data_url TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS native_bookmarks (
                 hash TEXT PRIMARY KEY,
                 namespace TEXT NOT NULL,
                 name TEXT NOT NULL,
                 note TEXT NOT NULL DEFAULT '',
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS marker_comments (
                 id TEXT PRIMARY KEY,
                 marker_id TEXT NOT NULL,
                 author TEXT NOT NULL,
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_marker_comments_marker_id
                 ON marker_comments (marker_id, created_at DESC);
             CREATE TABLE IF NOT EXISTS sol_news (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 body TEXT NOT NULL,
                 author TEXT NOT NULL,
                 published_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sol_kv_store (
                 key TEXT PRIMARY KEY,
                 value_json TEXT NOT NULL,
                 updated_at INTEGER NOT NULL
             );",
        )
        .context("initialize sqlite schema")?;

        let notif_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM sol_notifications", [], |r| r.get(0))?;
        if notif_count == 0 {
            conn.execute(
                "INSERT INTO sol_notifications (id, title, body, read, created_at)
                 VALUES (?1, ?2, ?3, 0, ?4)",
                params![
                    "welcome-v03",
                    "State of Leonida v0.3",
                    "Interactive map, media archive, and Screenshot Challenge are live.",
                    now_unix_millis()
                ],
            )?;
        }

        let news_count: i64 = conn.query_row("SELECT COUNT(*) FROM sol_news", [], |r| r.get(0))?;
        if news_count == 0 {
            conn.execute(
                "INSERT INTO sol_news (id, title, body, author, published_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "news-v08",
                    "State of Leonida v0.8 Map Studio",
                    "Heatmap, Marker Motion, Landmark Discussions, and Admin Studio are now live.",
                    "Leonida Team",
                    now_unix_millis()
                ],
            )?;
        }

        let contact_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM phone_contacts", [], |r| r.get(0))?;
        if contact_count == 0 {
            let defaults = [
                (
                    "lucia",
                    "Lucia Caminos",
                    "Partner",
                    "Vice Beach",
                    "#ff4d6d",
                    "Co-pilot and Leonida field scout.",
                ),
                (
                    "jason",
                    "Jason Duval",
                    "Partner",
                    "Keys",
                    "#4cc9f0",
                    "Reconnaissance & vehicle handling specialist.",
                ),
                (
                    "cal",
                    "Cal Hampton",
                    "Informant",
                    "Grassrivers",
                    "#f77f00",
                    "Tracks radio chatter and swamp sightings.",
                ),
                (
                    "dre",
                    "Dre'Quan Priest",
                    "Record Exec",
                    "Downtown",
                    "#7209b7",
                    "Only Raw Records studio & Vice City nightlife.",
                ),
                (
                    "boobie",
                    "Boobie Ike",
                    "Broker",
                    "Port Vice City",
                    "#2ec4b6",
                    "Port Vice City & real estate intel.",
                ),
            ];
            for (id, name, role, district, color, bio) in defaults {
                conn.execute(
                    "INSERT INTO phone_contacts (id, name, role, district, avatar_color, bio)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, name, role, district, color, bio],
                )?;
            }
            let now = now_unix_millis();
            conn.execute(
                "INSERT INTO phone_messages (id, contact_id, sender, text, timestamp, unread)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                params![
                    "msg-init-1",
                    "cal",
                    "Cal Hampton",
                    "Check the Grassrivers airboat dock — spotted a new landmark.",
                    now
                ],
            )?;
            conn.execute(
                "INSERT INTO phone_messages (id, contact_id, sender, text, timestamp, unread)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                params![
                    "msg-init-2",
                    "lucia",
                    "Lucia Caminos",
                    "Meet at Vice Beach boardwalk when you're ready.",
                    now - 60_000
                ],
            )?;
        }

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

    pub fn list_notifications(&self) -> Result<Vec<SolNotification>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, title, body, read, created_at FROM sol_notifications ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let read_int: i64 = row.get(3)?;
            Ok(SolNotification {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                read: read_int != 0,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn mark_notifications_read(&self, id: Option<&str>) -> Result<usize> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let updated = match id {
            Some(target_id) => conn.execute(
                "UPDATE sol_notifications SET read = 1 WHERE id = ?1 AND read = 0",
                params![target_id],
            )?,
            None => conn.execute("UPDATE sol_notifications SET read = 1 WHERE read = 0", [])?,
        };
        Ok(updated)
    }

    pub fn record_challenge_score(&self, entry: NewLeaderboardEntry) -> Result<LeaderboardEntry> {
        let item = LeaderboardEntry {
            id: Uuid::new_v4().to_string(),
            game: entry.game,
            player: entry.player,
            entry_id: entry.entry_id,
            score: entry.score,
            distance_m: entry.distance_m,
            grade: entry.grade,
            guess_x: entry.guess_x,
            guess_y: entry.guess_y,
            target_x: entry.target_x,
            target_y: entry.target_y,
            created_at: now_unix_millis(),
        };
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO sol_leaderboard (
                 id, game, player, entry_id, score, distance_m, grade,
                 guess_x, guess_y, target_x, target_y, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                item.id,
                item.game,
                item.player,
                item.entry_id,
                item.score,
                item.distance_m,
                item.grade,
                item.guess_x,
                item.guess_y,
                item.target_x,
                item.target_y,
                item.created_at
            ],
        )?;
        Ok(item)
    }

    pub fn leaderboard(&self, game: &str, limit: usize) -> Result<Vec<LeaderboardEntry>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, game, player, entry_id, score, distance_m, grade,
                    guess_x, guess_y, target_x, target_y, created_at
             FROM sol_leaderboard
             WHERE game = ?1
             ORDER BY score DESC, distance_m ASC, created_at ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![game, limit as i64], |row| {
            Ok(LeaderboardEntry {
                id: row.get(0)?,
                game: row.get(1)?,
                player: row.get(2)?,
                entry_id: row.get(3)?,
                score: row.get(4)?,
                distance_m: row.get(5)?,
                grade: row.get(6)?,
                guess_x: row.get(7)?,
                guess_y: row.get(8)?,
                target_x: row.get(9)?,
                target_y: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn game_stats(&self, game: &str) -> Result<SolGameStats> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let (total_plays, best_score, avg_score, avg_distance_m): (
            i64,
            Option<i64>,
            Option<f64>,
            Option<f64>,
        ) = conn.query_row(
            "SELECT COUNT(*), MAX(score), AVG(score), AVG(distance_m)
             FROM sol_leaderboard WHERE game = ?1",
            params![game],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        Ok(SolGameStats {
            game: game.to_string(),
            total_plays: total_plays.max(0) as usize,
            best_score: best_score.unwrap_or(0),
            avg_score: avg_score.unwrap_or(0.0),
            avg_distance_m: avg_distance_m.unwrap_or(0.0),
        })
    }

    pub fn list_projects(&self) -> Result<Vec<PhotogrammetryProject>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, image_name, width, height, observations_json, pose_json, frustum_json, created_at, updated_at
             FROM photogrammetry_projects
             ORDER BY updated_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let obs_raw: String = row.get(5)?;
            let pose_raw: Option<String> = row.get(6)?;
            let frustum_raw: Option<String> = row.get(7)?;
            Ok(PhotogrammetryProject {
                id: row.get(0)?,
                name: row.get(1)?,
                image_name: row.get(2)?,
                width: row.get(3)?,
                height: row.get(4)?,
                observations: serde_json::from_str(&obs_raw).unwrap_or_default(),
                pose: pose_raw.and_then(|s| serde_json::from_str(&s).ok()),
                frustum: frustum_raw.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn save_project(&self, input: SaveProjectInput) -> Result<PhotogrammetryProject> {
        let now = now_unix_millis();
        let id = input
            .id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let obs_json = serde_json::to_string(&input.observations)?;
        let pose_json = input.pose.as_ref().map(serde_json::to_string).transpose()?;
        let frustum_json = input
            .frustum
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let existing_created: Option<i64> = conn
            .query_row(
                "SELECT created_at FROM photogrammetry_projects WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        let created_at = existing_created.unwrap_or(now);

        conn.execute(
            "INSERT INTO photogrammetry_projects (
                 id, name, image_name, width, height, observations_json, pose_json, frustum_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 image_name = excluded.image_name,
                 width = excluded.width,
                 height = excluded.height,
                 observations_json = excluded.observations_json,
                 pose_json = excluded.pose_json,
                 frustum_json = excluded.frustum_json,
                 updated_at = excluded.updated_at",
            params![
                id,
                input.name,
                input.image_name,
                input.width,
                input.height,
                obs_json,
                pose_json,
                frustum_json,
                created_at,
                now
            ],
        )?;

        Ok(PhotogrammetryProject {
            id,
            name: input.name,
            image_name: input.image_name,
            width: input.width,
            height: input.height,
            observations: input.observations,
            pose: input.pose,
            frustum: input.frustum,
            created_at,
            updated_at: now,
        })
    }

    pub fn delete_project(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let affected = conn.execute(
            "DELETE FROM photogrammetry_projects WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_companion_progress(&self) -> Result<HashMap<String, bool>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare("SELECT id, completed FROM companion_progress")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let completed: i64 = row.get(1)?;
            Ok((id, completed != 0))
        })?;
        let mut out = HashMap::new();
        for row in rows {
            let (id, completed) = row?;
            out.insert(id, completed);
        }
        Ok(out)
    }

    pub fn set_companion_item(&self, id: &str, completed: Option<bool>) -> Result<bool> {
        let now = now_unix_millis();
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let next = match completed {
            Some(v) => v,
            None => {
                let current: Option<i64> = conn
                    .query_row(
                        "SELECT completed FROM companion_progress WHERE id = ?1",
                        params![id],
                        |row| row.get(0),
                    )
                    .optional()?;
                current.unwrap_or(0) == 0
            }
        };
        conn.execute(
            "INSERT INTO companion_progress (id, completed, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
                 completed = excluded.completed,
                 updated_at = excluded.updated_at",
            params![id, i64::from(next), now],
        )?;
        Ok(next)
    }

    pub fn import_companion_progress(&self, entries: &[(String, bool)]) -> Result<usize> {
        let now = now_unix_millis();
        let mut conn = self.conn.lock().expect("marker db mutex poisoned");
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO companion_progress (id, completed, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                     completed = excluded.completed,
                     updated_at = excluded.updated_at",
            )?;
            for (id, completed) in entries {
                stmt.execute(params![id, i64::from(*completed), now])?;
            }
        }
        tx.commit()?;
        Ok(entries.len())
    }

    pub fn list_phone_contacts(&self) -> Result<Vec<PhoneContact>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, role, district, avatar_color, bio FROM phone_contacts ORDER BY rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PhoneContact {
                id: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                district: row.get(3)?,
                avatar_color: row.get(4)?,
                bio: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn list_phone_messages(&self, contact_id: Option<&str>) -> Result<Vec<PhoneMessage>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut out = Vec::new();
        if let Some(cid) = contact_id {
            let mut stmt = conn.prepare(
                "SELECT id, contact_id, sender, text, timestamp, unread
                 FROM phone_messages WHERE contact_id = ?1 ORDER BY timestamp ASC, rowid ASC",
            )?;
            let rows = stmt.query_map(params![cid], map_phone_message)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, contact_id, sender, text, timestamp, unread
                 FROM phone_messages ORDER BY timestamp ASC, rowid ASC",
            )?;
            let rows = stmt.query_map([], map_phone_message)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn create_phone_message(&self, input: CreatePhoneMessageInput) -> Result<PhoneMessage> {
        let msg = PhoneMessage {
            id: Uuid::new_v4().to_string(),
            contact_id: input.contact_id,
            sender: input.sender.unwrap_or_else(|| "me".to_string()),
            text: input.text,
            timestamp: now_unix_millis(),
            unread: false,
        };
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO phone_messages (id, contact_id, sender, text, timestamp, unread)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![msg.id, msg.contact_id, msg.sender, msg.text, msg.timestamp],
        )?;
        Ok(msg)
    }

    pub fn list_phone_notes(&self) -> Result<Vec<PhoneNote>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, title, body, x, y, updated_at FROM phone_notes ORDER BY updated_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PhoneNote {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                x: row.get(3)?,
                y: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn save_phone_note(&self, input: SavePhoneNoteInput) -> Result<PhoneNote> {
        let now = now_unix_millis();
        let id = input
            .id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let note = PhoneNote {
            id,
            title: input.title,
            body: input.body,
            x: input.x,
            y: input.y,
            updated_at: now,
        };
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO phone_notes (id, title, body, x, y, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 body = excluded.body,
                 x = excluded.x,
                 y = excluded.y,
                 updated_at = excluded.updated_at",
            params![
                note.id,
                note.title,
                note.body,
                note.x,
                note.y,
                note.updated_at
            ],
        )?;
        Ok(note)
    }

    pub fn delete_phone_note(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let affected = conn.execute("DELETE FROM phone_notes WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn list_phone_photos(&self) -> Result<Vec<SnapmaticPhoto>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, title, district, x, y, filter, caption, image_data_url, created_at
             FROM phone_photos ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SnapmaticPhoto {
                id: row.get(0)?,
                title: row.get(1)?,
                district: row.get(2)?,
                x: row.get(3)?,
                y: row.get(4)?,
                filter: row.get(5)?,
                caption: row.get(6)?,
                image_data_url: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn save_phone_photo(&self, input: SavePhonePhotoInput) -> Result<SnapmaticPhoto> {
        let now = now_unix_millis();
        let id = input
            .id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let photo = SnapmaticPhoto {
            id,
            title: input.title,
            district: input.district.unwrap_or_else(|| "Leonida".to_string()),
            x: input.x,
            y: input.y,
            filter: input.filter.unwrap_or_else(|| "none".to_string()),
            caption: input.caption.unwrap_or_default(),
            image_data_url: input.image_data_url.unwrap_or_default(),
            created_at: now,
        };
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO phone_photos (id, title, district, x, y, filter, caption, image_data_url, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 district = excluded.district,
                 x = excluded.x,
                 y = excluded.y,
                 filter = excluded.filter,
                 caption = excluded.caption,
                 image_data_url = excluded.image_data_url",
            params![
                photo.id,
                photo.title,
                photo.district,
                photo.x,
                photo.y,
                photo.filter,
                photo.caption,
                photo.image_data_url,
                photo.created_at
            ],
        )?;
        Ok(photo)
    }

    pub fn delete_phone_photo(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let affected = conn.execute("DELETE FROM phone_photos WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn list_native_bookmarks(&self) -> Result<Vec<NativeBookmark>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT hash, namespace, name, note, created_at
             FROM native_bookmarks ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(NativeBookmark {
                hash: row.get(0)?,
                namespace: row.get(1)?,
                name: row.get(2)?,
                note: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn save_native_bookmark(&self, input: SaveNativeBookmarkInput) -> Result<NativeBookmark> {
        let now = now_unix_millis();
        let bookmark = NativeBookmark {
            hash: input.hash,
            namespace: input.namespace.unwrap_or_default(),
            name: input.name.unwrap_or_default(),
            note: input.note.unwrap_or_default(),
            created_at: now,
        };
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO native_bookmarks (hash, namespace, name, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(hash) DO UPDATE SET
                 namespace = excluded.namespace,
                 name = excluded.name,
                 note = excluded.note",
            params![
                bookmark.hash,
                bookmark.namespace,
                bookmark.name,
                bookmark.note,
                bookmark.created_at
            ],
        )?;
        Ok(bookmark)
    }

    pub fn delete_native_bookmark(&self, hash: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let affected = conn.execute(
            "DELETE FROM native_bookmarks WHERE hash = ?1",
            params![hash],
        )?;
        Ok(affected > 0)
    }

    pub fn list_marker_comments(&self, marker_id: &str) -> Result<Vec<MarkerComment>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, marker_id, author, body, created_at
             FROM marker_comments WHERE marker_id = ?1
             ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map(params![marker_id], |row| {
            Ok(MarkerComment {
                id: row.get(0)?,
                marker_id: row.get(1)?,
                author: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn post_marker_comment(
        &self,
        marker_id: &str,
        author: &str,
        body: &str,
    ) -> Result<MarkerComment> {
        let comment = MarkerComment {
            id: Uuid::new_v4().to_string(),
            marker_id: marker_id.to_string(),
            author: author.to_string(),
            body: body.to_string(),
            created_at: now_unix_millis(),
        };
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO marker_comments (id, marker_id, author, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                comment.id,
                comment.marker_id,
                comment.author,
                comment.body,
                comment.created_at
            ],
        )?;
        Ok(comment)
    }

    pub fn marker_discussion_stats(&self) -> Result<MarkerDiscussionStats> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let total_comments: i64 =
            conn.query_row("SELECT COUNT(*) FROM marker_comments", [], |r| r.get(0))?;
        let discussed_markers: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT marker_id) FROM marker_comments",
            [],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT marker_id, COUNT(*) as cnt
             FROM marker_comments GROUP BY marker_id ORDER BY cnt DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MarkerCommentCount {
                marker_id: row.get(0)?,
                count: row.get::<_, i64>(1)?.max(0) as usize,
            })
        })?;
        let mut by_marker = Vec::new();
        for row in rows {
            by_marker.push(row?);
        }
        Ok(MarkerDiscussionStats {
            total_comments: total_comments.max(0) as usize,
            discussed_markers: discussed_markers.max(0) as usize,
            by_marker,
        })
    }

    pub fn list_news(&self) -> Result<Vec<SolNewsItem>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, title, body, author, published_at
             FROM sol_news ORDER BY published_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SolNewsItem {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                author: row.get(3)?,
                published_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn save_news(
        &self,
        id: Option<String>,
        title: String,
        body: String,
        author: String,
    ) -> Result<SolNewsItem> {
        let item = SolNewsItem {
            id: id
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            title,
            body,
            author,
            published_at: now_unix_millis(),
        };
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO sol_news (id, title, body, author, published_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 body = excluded.body,
                 author = excluded.author,
                 published_at = excluded.published_at",
            params![
                item.id,
                item.title,
                item.body,
                item.author,
                item.published_at
            ],
        )?;
        Ok(item)
    }

    pub fn get_kv(&self, key: &str) -> Result<Option<Value>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let raw: Option<String> = conn
            .query_row(
                "SELECT value_json FROM sol_kv_store WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn set_kv(&self, key: &str, value: &Value) -> Result<()> {
        let raw = serde_json::to_string(value)?;
        let now = now_unix_millis();
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        conn.execute(
            "INSERT INTO sol_kv_store (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                 value_json = excluded.value_json,
                 updated_at = excluded.updated_at",
            params![key, raw, now],
        )?;
        Ok(())
    }

    pub fn admin_db_overview(&self) -> Result<Vec<AdminTableStat>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name ASC",
        )?;
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut tables = Vec::with_capacity(names.len());
        for name in names {
            let safe_name = name.replace('"', "\"\"");
            let count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM \"{safe_name}\""), [], |r| {
                    r.get(0)
                })?;
            tables.push(AdminTableStat {
                name,
                row_count: count.max(0) as usize,
            });
        }
        Ok(tables)
    }

    pub fn admin_db_integrity(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare("PRAGMA integrity_check")?;
        let checks = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(checks)
    }

    pub fn admin_stats(&self) -> Result<AdminStats> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let count = |table: &str| -> Result<usize> {
            let c: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            Ok(c.max(0) as usize)
        };
        Ok(AdminStats {
            markers_count: count("user_markers")?,
            comments_count: count("marker_comments")?,
            leaderboard_count: count("sol_leaderboard")?,
            projects_count: count("photogrammetry_projects")?,
            notes_count: count("phone_notes")?,
            photos_count: count("phone_photos")?,
            bookmarks_count: count("native_bookmarks")?,
            news_count: count("sol_news")?,
        })
    }

    pub fn admin_users(&self) -> Result<Vec<AdminUserSummary>> {
        let conn = self.conn.lock().expect("marker db mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT player, COUNT(*), MAX(score) FROM sol_leaderboard GROUP BY player ORDER BY MAX(score) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AdminUserSummary {
                name: row.get(0)?,
                role: "Explorer".to_string(),
                plays: row.get::<_, i64>(1)?.max(0) as usize,
                best_score: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerComment {
    pub id: String,
    pub marker_id: String,
    pub author: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerCommentCount {
    pub marker_id: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerDiscussionStats {
    pub total_comments: usize,
    pub discussed_markers: usize,
    pub by_marker: Vec<MarkerCommentCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolNewsItem {
    pub id: String,
    pub title: String,
    pub body: String,
    pub author: String,
    pub published_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminTableStat {
    pub name: String,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStats {
    pub markers_count: usize,
    pub comments_count: usize,
    pub leaderboard_count: usize,
    pub projects_count: usize,
    pub notes_count: usize,
    pub photos_count: usize,
    pub bookmarks_count: usize,
    pub news_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserSummary {
    pub name: String,
    pub role: String,
    pub plays: usize,
    pub best_score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBookmark {
    pub hash: String,
    pub namespace: String,
    pub name: String,
    pub note: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNativeBookmarkInput {
    pub hash: String,
    pub namespace: Option<String>,
    pub name: Option<String>,
    pub note: Option<String>,
}

fn map_phone_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<PhoneMessage> {
    let unread_int: i64 = row.get(5)?;
    Ok(PhoneMessage {
        id: row.get(0)?,
        contact_id: row.get(1)?,
        sender: row.get(2)?,
        text: row.get(3)?,
        timestamp: row.get(4)?,
        unread: unread_int != 0,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneContact {
    pub id: String,
    pub name: String,
    pub role: String,
    pub district: String,
    pub avatar_color: String,
    pub bio: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneMessage {
    pub id: String,
    pub contact_id: String,
    pub sender: String,
    pub text: String,
    pub timestamp: i64,
    pub unread: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePhoneMessageInput {
    #[serde(alias = "contact_id")]
    pub contact_id: String,
    pub sender: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneNote {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePhoneNoteInput {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapmaticPhoto {
    pub id: String,
    pub title: String,
    pub district: String,
    pub x: f64,
    pub y: f64,
    pub filter: String,
    pub caption: String,
    pub image_data_url: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePhonePhotoInput {
    pub id: Option<String>,
    pub title: String,
    pub district: Option<String>,
    pub x: f64,
    pub y: f64,
    pub filter: Option<String>,
    pub caption: Option<String>,
    #[serde(alias = "image_data_url")]
    pub image_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotogrammetryProject {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_name: Option<String>,
    pub width: f64,
    pub height: f64,
    pub observations: Vec<CameraObs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pose: Option<CameraPose>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frustum: Option<Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(alias = "image_name")]
    pub image_name: Option<String>,
    pub width: f64,
    pub height: f64,
    pub observations: Vec<CameraObs>,
    pub pose: Option<CameraPose>,
    pub frustum: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolNotification {
    pub id: String,
    pub title: String,
    pub body: String,
    pub read: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewLeaderboardEntry {
    pub game: String,
    pub player: String,
    pub entry_id: Option<String>,
    pub score: i64,
    pub distance_m: f64,
    pub grade: String,
    pub guess_x: f64,
    pub guess_y: f64,
    pub target_x: f64,
    pub target_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub id: String,
    pub game: String,
    pub player: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_id: Option<String>,
    pub score: i64,
    pub distance_m: f64,
    pub grade: String,
    pub guess_x: f64,
    pub guess_y: f64,
    pub target_x: f64,
    pub target_y: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolGameStats {
    pub game: String,
    pub total_plays: usize,
    pub best_score: i64,
    pub avg_score: f64,
    pub avg_distance_m: f64,
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
