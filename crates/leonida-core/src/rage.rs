//! RAGE archive/resource header decoding, JOAAT hashing, and `handling.meta` XML codec.
//!
//! Strictly compliant: no embedded keys, no DRM bypass.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

pub const RPF7_MAGIC: u32 = 0x5250_4637;
pub const RPF8_MAGIC: u32 = 0x5250_4638;
pub const RSC7_MAGIC: u32 = 0x3743_5352;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RpfVersion {
    Rpf7,
    Rpf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RpfEncryption {
    None,
    Open,
    Aes,
    Ng,
    Unknown(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpfHeader {
    pub version: RpfVersion,
    pub entry_count: u32,
    pub names_length: u32,
    pub encryption: RpfEncryption,
    pub encryption_raw: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rsc7Header {
    pub version: u32,
    pub system_flags: u32,
    pub graphics_flags: u32,
    pub system_size: u64,
    pub graphics_size: u64,
}

/// Unified header decode result for RPF7, RPF8, and RSC7 byte slices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RageHeader {
    Rpf(RpfHeader),
    Rsc7(Rsc7Header),
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Parse an RPF7 or RPF8 archive header (requires at least 16 bytes).
pub fn parse_rpf_header(bytes: &[u8]) -> Result<RpfHeader, CoreError> {
    if bytes.len() < 16 {
        return Err(CoreError::InvalidHeader(format!(
            "expected at least 16 bytes for RPF header, got {}",
            bytes.len()
        )));
    }

    let magic_le = read_u32_le(bytes, 0).unwrap_or(0);
    let magic_be = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

    let version = if magic_le == RPF7_MAGIC || magic_be == RPF7_MAGIC {
        RpfVersion::Rpf7
    } else if magic_le == RPF8_MAGIC || magic_be == RPF8_MAGIC {
        RpfVersion::Rpf8
    } else {
        return Err(CoreError::InvalidHeader(format!(
            "unrecognized RPF magic: 0x{magic_le:08x}"
        )));
    };

    let entry_count = read_u32_le(bytes, 4).unwrap_or(0);
    let names_length = read_u32_le(bytes, 8).unwrap_or(0);
    let encryption_raw = read_u32_le(bytes, 12).unwrap_or(0);

    let encryption = match encryption_raw {
        0 => RpfEncryption::None,
        0x4E45_504F => RpfEncryption::Open,
        0x0FFF_FFF9 => RpfEncryption::Aes,
        0x0FFF_FFFF | 0x0FEF_FFFF => RpfEncryption::Ng,
        other => RpfEncryption::Unknown(other),
    };

    Ok(RpfHeader {
        version,
        entry_count,
        names_length,
        encryption,
        encryption_raw,
    })
}

/// Compute RSC7 segment byte size from a 32-bit `system_flags` or `graphics_flags` word.
pub fn compute_segment_size(flags: u32) -> u64 {
    let shift = flags & 0x0F;
    let base = 0x2000_u64.checked_shl(shift).unwrap_or(u64::MAX);

    let s0 = u64::from((flags >> 27) & 0x01).saturating_mul(base / 16);
    let s1 = u64::from((flags >> 26) & 0x01).saturating_mul(base / 8);
    let s2 = u64::from((flags >> 25) & 0x01).saturating_mul(base / 4);
    let s3 = u64::from((flags >> 24) & 0x01).saturating_mul(base / 2);
    let s4 = u64::from((flags >> 17) & 0x7F).saturating_mul(base);
    let s5 = u64::from((flags >> 11) & 0x3F).saturating_mul(base.saturating_mul(2));
    let s6 = u64::from((flags >> 7) & 0x0F).saturating_mul(base.saturating_mul(4));
    let s7 = u64::from((flags >> 5) & 0x03).saturating_mul(base.saturating_mul(8));
    let s8 = u64::from((flags >> 4) & 0x01).saturating_mul(base.saturating_mul(16));

    s0.saturating_add(s1)
        .saturating_add(s2)
        .saturating_add(s3)
        .saturating_add(s4)
        .saturating_add(s5)
        .saturating_add(s6)
        .saturating_add(s7)
        .saturating_add(s8)
}

/// Parse an RSC7 resource header (requires at least 16 bytes).
pub fn parse_rsc7_header(bytes: &[u8]) -> Result<Rsc7Header, CoreError> {
    if bytes.len() < 16 {
        return Err(CoreError::InvalidHeader(format!(
            "expected at least 16 bytes for RSC7 header, got {}",
            bytes.len()
        )));
    }

    let magic = read_u32_le(bytes, 0).unwrap_or(0);
    if magic != RSC7_MAGIC {
        return Err(CoreError::InvalidHeader(format!(
            "expected RSC7 magic 0x{RSC7_MAGIC:08x}, got 0x{magic:08x}"
        )));
    }

    let version = read_u32_le(bytes, 4).unwrap_or(0);
    let system_flags = read_u32_le(bytes, 8).unwrap_or(0);
    let graphics_flags = read_u32_le(bytes, 12).unwrap_or(0);

    Ok(Rsc7Header {
        version,
        system_flags,
        graphics_flags,
        system_size: compute_segment_size(system_flags),
        graphics_size: compute_segment_size(graphics_flags),
    })
}

/// Auto-detect and parse an RPF7, RPF8, or RSC7 header from raw bytes.
pub fn parse_rage_header(bytes: &[u8]) -> Result<RageHeader, CoreError> {
    if let Ok(rpf) = parse_rpf_header(bytes) {
        return Ok(RageHeader::Rpf(rpf));
    }
    if let Ok(rsc7) = parse_rsc7_header(bytes) {
        return Ok(RageHeader::Rsc7(rsc7));
    }
    Err(CoreError::InvalidHeader(
        "bytes do not match RPF7, RPF8, or RSC7 header".to_string(),
    ))
}

/// Jenkins one-at-a-time 32-bit hash (ASCII lowercase, RAGE convention).
pub fn joaat(name: &str) -> u32 {
    let mut hash: u32 = 0;
    for b in name.bytes() {
        let c = b.to_ascii_lowercase();
        hash = hash.wrapping_add(u32::from(c));
        hash = hash.wrapping_add(hash << 10);
        hash ^= hash >> 6;
    }
    hash = hash.wrapping_add(hash << 3);
    hash ^= hash >> 11;
    hash = hash.wrapping_add(hash << 15);
    hash
}

/// Compute the RAGE NG key table index `0..101` for a given entry name and file size.
pub fn compute_ng_key_index(name: &str, file_size: u64) -> usize {
    let h = u64::from(joaat(name));
    (h.wrapping_add(file_size).wrapping_add(101 - 40) % 101) as usize
}

/// Vehicle handling parameters from `handling.meta`.
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

impl Default for HandlingItem {
    fn default() -> Self {
        Self {
            handling_name: String::new(),
            f_mass: 1500.0,
            f_initial_drag_coeff: 8.0,
            f_drive_bias_front: 0.0,
            n_initial_drive_gears: 6,
            f_initial_drive_force: 0.3,
            f_drive_max_flat_vel: 160.0,
            f_brake_force: 1.0,
            f_traction_curve_max: 2.2,
            f_suspension_force: 2.0,
        }
    }
}

fn extract_tag_text<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let rest = block.get(start..)?;
    let end = rest.find(&close)?;
    Some(rest[..end].trim())
}

fn extract_attr_value<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let needle = format!("<{tag}");
    let start = block.find(&needle)?;
    let after_tag = block.get(start + needle.len()..)?;
    let tag_end = after_tag.find('>')?;
    let attrs = &after_tag[..tag_end];

    let val_idx = attrs.find("value=")?;
    let after_eq = attrs.get(val_idx + "value=".len()..)?.trim_start();
    let quote = after_eq.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &after_eq[1..];
    let end_quote = inner.find(quote)?;
    Some(inner[..end_quote].trim())
}

fn parse_f64_attr(block: &str, tag: &str, default_val: f64) -> Result<f64, CoreError> {
    let Some(raw) = extract_attr_value(block, tag) else {
        return Ok(default_val);
    };
    raw.parse::<f64>()
        .map_err(|e| CoreError::InvalidHandlingMeta(format!("{tag}='{raw}': {e}")))
}

fn parse_u32_attr(block: &str, tag: &str, default_val: u32) -> Result<u32, CoreError> {
    let Some(raw) = extract_attr_value(block, tag) else {
        return Ok(default_val);
    };
    raw.parse::<u32>()
        .map_err(|e| CoreError::InvalidHandlingMeta(format!("{tag}='{raw}': {e}")))
}

/// Parse `<Item ...>...</Item>` entries from a `handling.meta` XML string.
pub fn parse_handling_meta(xml: &str) -> Result<Vec<HandlingItem>, CoreError> {
    let mut items = Vec::new();
    let mut cursor = xml;

    while let Some(item_start) = cursor.find("<Item") {
        let after_start = &cursor[item_start..];
        let Some(open_end) = after_start.find('>') else {
            return Err(CoreError::InvalidHandlingMeta(
                "unclosed <Item> start tag".to_string(),
            ));
        };
        let body_start = &after_start[open_end + 1..];
        let Some(item_end) = body_start.find("</Item>") else {
            return Err(CoreError::InvalidHandlingMeta(
                "missing </Item> closing tag".to_string(),
            ));
        };
        let block = &body_start[..item_end];
        cursor = &body_start[item_end + "</Item>".len()..];

        let Some(name) = extract_tag_text(block, "handlingName") else {
            continue;
        };
        if name.is_empty() {
            return Err(CoreError::InvalidHandlingMeta(
                "empty <handlingName>".to_string(),
            ));
        }

        let defaults = HandlingItem::default();
        items.push(HandlingItem {
            handling_name: name.to_string(),
            f_mass: parse_f64_attr(block, "fMass", defaults.f_mass)?,
            f_initial_drag_coeff: parse_f64_attr(
                block,
                "fInitialDragCoeff",
                defaults.f_initial_drag_coeff,
            )?,
            f_drive_bias_front: parse_f64_attr(
                block,
                "fDriveBiasFront",
                defaults.f_drive_bias_front,
            )?,
            n_initial_drive_gears: parse_u32_attr(
                block,
                "nInitialDriveGears",
                defaults.n_initial_drive_gears,
            )?,
            f_initial_drive_force: parse_f64_attr(
                block,
                "fInitialDriveForce",
                defaults.f_initial_drive_force,
            )?,
            f_drive_max_flat_vel: parse_f64_attr(
                block,
                "fDriveMaxFlatVel",
                defaults.f_drive_max_flat_vel,
            )?,
            f_brake_force: parse_f64_attr(block, "fBrakeForce", defaults.f_brake_force)?,
            f_traction_curve_max: parse_f64_attr(
                block,
                "fTractionCurveMax",
                defaults.f_traction_curve_max,
            )?,
            f_suspension_force: parse_f64_attr(
                block,
                "fSuspensionForce",
                defaults.f_suspension_force,
            )?,
        });
    }

    Ok(items)
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Serialize [`HandlingItem`] records into a `handling.meta` XML document.
pub fn export_handling_meta(items: &[HandlingItem]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CHandlingDataMgr>\n  <HandlingData>\n",
    );

    for item in items {
        xml.push_str("    <Item type=\"CHandlingData\">\n");
        xml.push_str(&format!(
            "      <handlingName>{}</handlingName>\n",
            escape_xml(&item.handling_name)
        ));
        xml.push_str(&format!("      <fMass value=\"{}\" />\n", item.f_mass));
        xml.push_str(&format!(
            "      <fInitialDragCoeff value=\"{}\" />\n",
            item.f_initial_drag_coeff
        ));
        xml.push_str(&format!(
            "      <fDriveBiasFront value=\"{}\" />\n",
            item.f_drive_bias_front
        ));
        xml.push_str(&format!(
            "      <nInitialDriveGears value=\"{}\" />\n",
            item.n_initial_drive_gears
        ));
        xml.push_str(&format!(
            "      <fInitialDriveForce value=\"{}\" />\n",
            item.f_initial_drive_force
        ));
        xml.push_str(&format!(
            "      <fDriveMaxFlatVel value=\"{}\" />\n",
            item.f_drive_max_flat_vel
        ));
        xml.push_str(&format!(
            "      <fBrakeForce value=\"{}\" />\n",
            item.f_brake_force
        ));
        xml.push_str(&format!(
            "      <fTractionCurveMax value=\"{}\" />\n",
            item.f_traction_curve_max
        ));
        xml.push_str(&format!(
            "      <fSuspensionForce value=\"{}\" />\n",
            item.f_suspension_force
        ));
        xml.push_str("    </Item>\n");
    }

    xml.push_str("  </HandlingData>\n</CHandlingDataMgr>\n");
    xml
}
