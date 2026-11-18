use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::{
    CreatePhoneMessageInput, PhoneContact, PhoneMessage, PhoneNote, SavePhoneNoteInput,
    SavePhonePhotoInput, SnapmaticPhoto,
};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ContactsResponse {
    pub contacts: Vec<PhoneContact>,
}

pub async fn list_contacts(
    State(state): State<AppState>,
) -> Result<Json<ContactsResponse>, AppError> {
    let contacts = state.markers.list_phone_contacts()?;
    Ok(Json(ContactsResponse { contacts }))
}

#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    pub contact_id: Option<String>,
    #[serde(rename = "contactId")]
    pub contact_id_camel: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<PhoneMessage>,
}

pub async fn list_messages(
    State(state): State<AppState>,
    Query(q): Query<MessagesQuery>,
) -> Result<Json<MessagesResponse>, AppError> {
    let cid = q.contact_id.or(q.contact_id_camel);
    let messages = state
        .markers
        .list_phone_messages(cid.as_deref().filter(|s| !s.trim().is_empty()))?;
    Ok(Json(MessagesResponse { messages }))
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: PhoneMessage,
}

pub async fn create_message(
    State(state): State<AppState>,
    Json(input): Json<CreatePhoneMessageInput>,
) -> Result<(StatusCode, Json<MessageResponse>), AppError> {
    if input.contact_id.trim().is_empty() {
        return Err(AppError::BadRequest("contactId must not be empty".into()));
    }
    if input.text.trim().is_empty() {
        return Err(AppError::BadRequest(
            "message text must not be empty".into(),
        ));
    }
    let message = state.markers.create_phone_message(input)?;
    Ok((StatusCode::CREATED, Json(MessageResponse { message })))
}

#[derive(Debug, Serialize)]
pub struct NotesResponse {
    pub notes: Vec<PhoneNote>,
}

pub async fn list_notes(State(state): State<AppState>) -> Result<Json<NotesResponse>, AppError> {
    let notes = state.markers.list_phone_notes()?;
    Ok(Json(NotesResponse { notes }))
}

#[derive(Debug, Serialize)]
pub struct NoteResponse {
    pub note: PhoneNote,
}

pub async fn save_note(
    State(state): State<AppState>,
    Json(input): Json<SavePhoneNoteInput>,
) -> Result<(StatusCode, Json<NoteResponse>), AppError> {
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("note title must not be empty".into()));
    }
    if input.x.is_some_and(|x| !x.is_finite()) || input.y.is_some_and(|y| !y.is_finite()) {
        return Err(AppError::BadRequest(
            "x and y must be finite numbers".into(),
        ));
    }
    let note = state.markers.save_phone_note(input)?;
    Ok((StatusCode::CREATED, Json(NoteResponse { note })))
}

#[derive(Debug, Deserialize)]
pub struct IdQuery {
    pub id: Option<String>,
}

fn extract_id(query: IdQuery, body: &Bytes) -> Result<String, AppError> {
    #[derive(Deserialize, Default)]
    struct IdBody {
        id: Option<String>,
    }
    let body_id = if body.is_empty() {
        None
    } else {
        serde_json::from_slice::<IdBody>(body)
            .map_err(|e| AppError::BadRequest(format!("invalid JSON body: {e}")))?
            .id
    };
    query
        .id
        .or(body_id)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("missing 'id'".into()))
}

pub async fn delete_note_query_or_body(
    State(state): State<AppState>,
    Query(query): Query<IdQuery>,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    let id = extract_id(query, &body)?;
    if !state.markers.delete_phone_note(&id)? {
        return Err(AppError::NotFound(format!("note '{id}' not found")));
    }
    Ok(Json(json!({ "deleted": true })))
}

pub async fn delete_note_path(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    if !state.markers.delete_phone_note(&id)? {
        return Err(AppError::NotFound(format!("note '{id}' not found")));
    }
    Ok(Json(json!({ "deleted": true })))
}

#[derive(Debug, Serialize)]
pub struct PhotosResponse {
    pub photos: Vec<SnapmaticPhoto>,
}

pub async fn list_photos(State(state): State<AppState>) -> Result<Json<PhotosResponse>, AppError> {
    let photos = state.markers.list_phone_photos()?;
    Ok(Json(PhotosResponse { photos }))
}

#[derive(Debug, Serialize)]
pub struct PhotoResponse {
    pub photo: SnapmaticPhoto,
}

pub async fn save_photo(
    State(state): State<AppState>,
    Json(input): Json<SavePhonePhotoInput>,
) -> Result<(StatusCode, Json<PhotoResponse>), AppError> {
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("photo title must not be empty".into()));
    }
    if !input.x.is_finite() || !input.y.is_finite() {
        return Err(AppError::BadRequest(
            "x and y must be finite numbers".into(),
        ));
    }
    let photo = state.markers.save_phone_photo(input)?;
    Ok((StatusCode::CREATED, Json(PhotoResponse { photo })))
}

pub async fn delete_photo_query_or_body(
    State(state): State<AppState>,
    Query(query): Query<IdQuery>,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    let id = extract_id(query, &body)?;
    if !state.markers.delete_phone_photo(&id)? {
        return Err(AppError::NotFound(format!("photo '{id}' not found")));
    }
    Ok(Json(json!({ "deleted": true })))
}

pub async fn delete_photo_path(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    if !state.markers.delete_phone_photo(&id)? {
        return Err(AppError::NotFound(format!("photo '{id}' not found")));
    }
    Ok(Json(json!({ "deleted": true })))
}
