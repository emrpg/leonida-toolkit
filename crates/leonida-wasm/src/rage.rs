use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpfHeader {
    pub version: String,
    pub entry_count: u32,
    pub names_length: u32,
    pub encryption: String,
    pub raw_encryption_tag: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rsc7Header {
    pub version: u32,
    pub system_flags: u32,
    pub graphics_flags: u32,
    pub system_size: u64,
    pub graphics_size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

pub fn parse_hex_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = hex
        .replace("0x", "")
        .replace("0X", "")
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if !cleaned.len().is_multiple_of(2) {
        return Err("hex string must have an even number of digits".to_string());
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    for i in (0..cleaned.len()).step_by(2) {
        let byte = u8::from_str_radix(&cleaned[i..i + 2], 16)
            .map_err(|e| format!("invalid hex byte: {e}"))?;
        out.push(byte);
    }
    Ok(out)
}

pub fn parse_rpf_header(bytes: &[u8]) -> Result<RpfHeader, String> {
    if bytes.len() < 16 {
        return Err("RPF header requires at least 16 bytes".to_string());
    }
    let magic_be = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let magic_le = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let version = if magic_be == 0x5250_4637 || magic_le == 0x5250_4637 {
        "RPF7"
    } else if magic_be == 0x5250_4638 || magic_le == 0x5250_4638 {
        "RPF8"
    } else {
        return Err(format!("invalid RPF magic: 0x{magic_be:08X}"));
    };

    let entry_count = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let names_length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let tag = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let encryption = match tag {
        0 => "None",
        0x4E45_504F => "Open",
        0x0FFF_FFF9 => "Aes",
        t if (t >> 24) == 0x0F => "Ng",
        _ => "Unknown",
    };

    Ok(RpfHeader {
        version: version.to_string(),
        entry_count,
        names_length,
        encryption: encryption.to_string(),
        raw_encryption_tag: tag,
    })
}

pub fn compute_segment_size(flags: u32) -> u64 {
    let base_shift = (flags >> 28) & 0xF;
    let base = 0x2000u64 << base_shift.min(10);
    let s4 = (flags & 0x1) as u64;
    let s3 = ((flags >> 1) & 0x3) as u64;
    let s2 = ((flags >> 3) & 0xF) as u64;
    let s1 = ((flags >> 7) & 0x3F) as u64;
    let s0 = ((flags >> 13) & 0x7F) as u64;
    let sub1 = ((flags >> 20) & 0x1F) as u64;
    let sub2 = ((flags >> 25) & 0x7) as u64;
    s4 * (base * 16)
        + s3 * (base * 8)
        + s2 * (base * 4)
        + s1 * (base * 2)
        + s0 * base
        + sub1 * (base / 2)
        + sub2 * (base / 4)
}

pub fn parse_rsc7_header(bytes: &[u8]) -> Result<Rsc7Header, String> {
    if bytes.len() < 16 {
        return Err("RSC7 header requires at least 16 bytes".to_string());
    }
    let magic_le = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let magic_be = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic_le != 0x3743_5352 && magic_be != 0x3743_5352 {
        return Err(format!("invalid RSC7 magic: 0x{magic_le:08X}"));
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let system_flags = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let graphics_flags = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    Ok(Rsc7Header {
        version,
        system_flags,
        graphics_flags,
        system_size: compute_segment_size(system_flags),
        graphics_size: compute_segment_size(graphics_flags),
    })
}

pub fn joaat(name: &str) -> u32 {
    let mut hash: u32 = 0;
    for &b in name.as_bytes() {
        let c = b.to_ascii_lowercase() as u32;
        hash = hash.wrapping_add(c);
        hash = hash.wrapping_add(hash << 10);
        hash ^= hash >> 6;
    }
    hash = hash.wrapping_add(hash << 3);
    hash ^= hash >> 11;
    hash = hash.wrapping_add(hash << 15);
    hash
}

pub fn compute_ng_key_index(name: &str, file_size: u64) -> u32 {
    let h = joaat(name) as u64;
    ((h ^ file_size) % 101) as u32
}

fn extract_tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(block[start..end].trim().to_string())
}

fn extract_tag_value(block: &str, tag: &str) -> Option<f64> {
    let prefix = format!("<{tag}");
    let start = block.find(&prefix)?;
    let rest = &block[start..];
    let end = rest.find('>')?;
    let elem = &rest[..end];
    let val_idx = elem.find("value=\"")? + "value=\"".len();
    let val_end = elem[val_idx..].find('"')? + val_idx;
    elem[val_idx..val_end].trim().parse::<f64>().ok()
}

pub fn parse_handling_meta(xml: &str) -> Result<Vec<HandlingItem>, String> {
    let mut items = Vec::new();
    let mut cursor = xml;
    while let Some(start) = cursor.find("<Item") {
        let after_start = &cursor[start..];
        let Some(end_rel) = after_start.find("</Item>") else {
            break;
        };
        let block = &after_start[..end_rel];
        if let Some(name) = extract_tag_text(block, "handlingName") {
            items.push(HandlingItem {
                handling_name: name,
                f_mass: extract_tag_value(block, "fMass").unwrap_or(1500.0),
                f_initial_drag_coeff: extract_tag_value(block, "fInitialDragCoeff").unwrap_or(8.0),
                f_drive_bias_front: extract_tag_value(block, "fDriveBiasFront").unwrap_or(0.0),
                n_initial_drive_gears: extract_tag_value(block, "nInitialDriveGears")
                    .map(|v| v.round().max(1.0) as u32)
                    .unwrap_or(6),
                f_initial_drive_force: extract_tag_value(block, "fInitialDriveForce")
                    .unwrap_or(0.28),
                f_drive_max_flat_vel: extract_tag_value(block, "fDriveMaxFlatVel").unwrap_or(150.0),
                f_brake_force: extract_tag_value(block, "fBrakeForce").unwrap_or(0.8),
                f_traction_curve_max: extract_tag_value(block, "fTractionCurveMax").unwrap_or(2.2),
                f_suspension_force: extract_tag_value(block, "fSuspensionForce").unwrap_or(2.0),
            });
        }
        cursor = &after_start[end_rel + "</Item>".len()..];
    }
    if items.is_empty() {
        return Err("no <Item> handling entries found in XML".to_string());
    }
    Ok(items)
}

pub fn export_handling_meta(items: &[HandlingItem]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CHandlingDataMgr>\n  <HandlingData>\n",
    );
    for item in items {
        out.push_str(&format!(
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
            item.handling_name,
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
    out.push_str("  </HandlingData>\n</CHandlingDataMgr>\n");
    out
}
