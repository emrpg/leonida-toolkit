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
