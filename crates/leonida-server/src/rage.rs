use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::AppError;

#[derive(Debug, Deserialize)]
pub struct InspectHeaderRequest {
    pub hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HandlingItem {
    pub handling_name: String,
    pub f_mass: f64,
    pub f_initial_drag_coeff: f64,
    pub f_drive_bias_front: f64,
    pub n_initial_drive_gears: u32,
    pub f_initial_drive_force: f64,
    pub f_drive_max_flat_vel: f64,
    pub f_brake_force: f64,
    pub f_traction_curve_max: f64,
    pub f_suspension_force: f64,
}

#[derive(Debug, Deserialize)]
pub struct HandlingParseRequest {
    pub xml: String,
}

#[derive(Debug, Serialize)]
pub struct HandlingParseResponse {
    pub items: Vec<HandlingItem>,
}

#[derive(Debug, Deserialize)]
pub struct HandlingExportRequest {
    pub items: Vec<HandlingItem>,
}

#[derive(Debug, Serialize)]
pub struct HandlingExportResponse {
    pub xml: String,
}

fn decode_hex_bytes(input: &str) -> Result<Vec<u8>, AppError> {
    let cleaned: String = input
        .replace("0x", "")
        .replace("0X", "")
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-' && *c != ',')
        .collect();
    if cleaned.is_empty() || !cleaned.len().is_multiple_of(2) {
        return Err(AppError::BadRequest(
            "hex input must contain an even number of hex digits".into(),
        ));
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let (chunks, _) = cleaned.as_bytes().as_chunks::<2>();
    for chunk in chunks {
        let s =
            std::str::from_utf8(chunk).map_err(|_| AppError::BadRequest("invalid utf8".into()))?;
        let byte = u8::from_str_radix(s, 16)
            .map_err(|_| AppError::BadRequest(format!("invalid hex byte '{s}'")))?;
        out.push(byte);
    }
    Ok(out)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn compute_segment_size(flags: u32) -> u64 {
    let base_size = 0x2000u64 << (flags & 0xF);
    let mut pages = 0u64;
    pages += ((flags >> 17) & 0x7F) as u64;
    pages += (((flags >> 11) & 0x3F) as u64) << 1;
    pages += (((flags >> 7) & 0xF) as u64) << 2;
    pages += (((flags >> 5) & 0x3) as u64) << 3;
    pages += (((flags >> 4) & 0x1) as u64) << 4;
    pages * base_size
}

fn rpf_encryption_label(tag: u32) -> String {
    match tag {
        0 => "None".to_string(),
        0x4E45_504F | 0x4F50_454E => "Open".to_string(),
        0x0FFF_FFF9 => "Aes".to_string(),
        0x0FEF_FFFF => "Ng".to_string(),
        other => format!("Unknown(0x{other:08X})"),
    }
}

pub async fn inspect_header(
    Json(req): Json<InspectHeaderRequest>,
) -> Result<Json<Value>, AppError> {
    let bytes = decode_hex_bytes(&req.hex)?;
    if bytes.len() < 16 {
        return Err(AppError::BadRequest(
            "header requires at least 16 bytes (32 hex chars)".into(),
        ));
    }

    let magic_be = read_u32_be(&bytes, 0);
    let magic_le = read_u32_le(&bytes, 0);

    // RPF7 (0x52504637) or RPF8 (0x52504638)
    if magic_be == 0x5250_4637
        || magic_be == 0x5250_4638
        || magic_le == 0x5250_4637
        || magic_le == 0x5250_4638
    {
        let big_endian = magic_be == 0x5250_4637 || magic_be == 0x5250_4638;
        let magic = if big_endian { magic_be } else { magic_le };
        let format = if magic == 0x5250_4637 { "RPF7" } else { "RPF8" };
        let entry_count = if big_endian {
            read_u32_be(&bytes, 4)
        } else {
            read_u32_le(&bytes, 4)
        };
        let names_length = if big_endian {
            read_u32_be(&bytes, 8)
        } else {
            read_u32_le(&bytes, 8)
        };
        let encryption_raw = if big_endian {
            read_u32_be(&bytes, 12)
        } else {
            read_u32_le(&bytes, 12)
        };

        return Ok(Json(json!({
            "format": format,
            "magic": format!("0x{magic:08X}"),
            "endianness": if big_endian { "big" } else { "little" },
            "entryCount": entry_count,
            "namesLength": names_length,
            "encryption": rpf_encryption_label(encryption_raw),
            "encryptionRaw": format!("0x{encryption_raw:08X}"),
        })));
    }

    // RSC7 (0x37435352 in LE, or 0x52534337 in BE ASCII)
    if magic_le == 0x3743_5352 || magic_be == 0x5253_4337 {
        let version = read_u32_le(&bytes, 4);
        let system_flags = read_u32_le(&bytes, 8);
        let graphics_flags = read_u32_le(&bytes, 12);
        return Ok(Json(json!({
            "format": "RSC7",
            "magic": format!("0x{magic_le:08X}"),
            "version": version,
            "systemFlags": format!("0x{system_flags:08X}"),
            "graphicsFlags": format!("0x{graphics_flags:08X}"),
            "systemSize": compute_segment_size(system_flags),
            "graphicsSize": compute_segment_size(graphics_flags),
        })));
    }

    Err(AppError::BadRequest(format!(
        "unrecognized RAGE header magic: 0x{magic_be:08X}"
    )))
}

fn extract_tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(block[start..end].trim().to_string())
}

fn extract_value_attr(block: &str, tag: &str) -> Option<f64> {
    let needle = format!("<{tag}");
    let start = block.find(&needle)?;
    let rest = &block[start..];
    let tag_end = rest.find('>')?;
    let tag_slice = &rest[..tag_end];
    let val_key = "value=\"";
    let v_start = tag_slice.find(val_key)? + val_key.len();
    let v_end = tag_slice[v_start..].find('"')? + v_start;
    tag_slice[v_start..v_end].trim().parse::<f64>().ok()
}

pub async fn parse_handling(
    Json(req): Json<HandlingParseRequest>,
) -> Result<Json<HandlingParseResponse>, AppError> {
    if req.xml.trim().is_empty() {
        return Err(AppError::BadRequest("xml must not be empty".into()));
    }

    let mut items = Vec::new();
    let mut cursor = req.xml.as_str();
    while let Some(item_start) = cursor.find("<Item") {
        let after_start = &cursor[item_start..];
        let Some(item_end) = after_start.find("</Item>") else {
            break;
        };
        let block = &after_start[..item_end];
        cursor = &after_start[item_end + "</Item>".len()..];

        if let Some(handling_name) = extract_tag_text(block, "handlingName") {
            items.push(HandlingItem {
                handling_name,
                f_mass: extract_value_attr(block, "fMass").unwrap_or(1500.0),
                f_initial_drag_coeff: extract_value_attr(block, "fInitialDragCoeff").unwrap_or(8.0),
                f_drive_bias_front: extract_value_attr(block, "fDriveBiasFront").unwrap_or(0.0),
                n_initial_drive_gears: extract_value_attr(block, "nInitialDriveGears")
                    .map(|v| v.round().clamp(1.0, 12.0) as u32)
                    .unwrap_or(6),
                f_initial_drive_force: extract_value_attr(block, "fInitialDriveForce")
                    .unwrap_or(0.3),
                f_drive_max_flat_vel: extract_value_attr(block, "fDriveMaxFlatVel")
                    .unwrap_or(160.0),
                f_brake_force: extract_value_attr(block, "fBrakeForce").unwrap_or(0.8),
                f_traction_curve_max: extract_value_attr(block, "fTractionCurveMax").unwrap_or(2.2),
                f_suspension_force: extract_value_attr(block, "fSuspensionForce").unwrap_or(2.0),
            });
        }
    }

    if items.is_empty() {
        return Err(AppError::BadRequest(
            "no valid <Item> with <handlingName> found in handling.meta XML".into(),
        ));
    }

    Ok(Json(HandlingParseResponse { items }))
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub async fn export_handling(
    Json(req): Json<HandlingExportRequest>,
) -> Result<Json<HandlingExportResponse>, AppError> {
    if req.items.is_empty() {
        return Err(AppError::BadRequest("items must not be empty".into()));
    }

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CHandlingDataMgr>\n  <HandlingData>\n",
    );
    for item in &req.items {
        xml.push_str(&format!(
            "    <Item type=\"CHandlingData\">\n\
             \x20     <handlingName>{}</handlingName>\n\
             \x20     <fMass value=\"{:.6}\" />\n\
             \x20     <fInitialDragCoeff value=\"{:.6}\" />\n\
             \x20     <fDriveBiasFront value=\"{:.6}\" />\n\
             \x20     <nInitialDriveGears value=\"{}\" />\n\
             \x20     <fInitialDriveForce value=\"{:.6}\" />\n\
             \x20     <fDriveMaxFlatVel value=\"{:.6}\" />\n\
             \x20     <fBrakeForce value=\"{:.6}\" />\n\
             \x20     <fTractionCurveMax value=\"{:.6}\" />\n\
             \x20     <fSuspensionForce value=\"{:.6}\" />\n\
             \x20   </Item>\n",
            escape_xml(&item.handling_name),
            item.f_mass,
            item.f_initial_drag_coeff,
            item.f_drive_bias_front,
            item.n_initial_drive_gears,
            item.f_initial_drive_force,
            item.f_drive_max_flat_vel,
            item.f_brake_force,
            item.f_traction_curve_max,
            item.f_suspension_force,
        ));
    }
    xml.push_str("  </HandlingData>\n</CHandlingDataMgr>\n");

    Ok(Json(HandlingExportResponse { xml }))
}
