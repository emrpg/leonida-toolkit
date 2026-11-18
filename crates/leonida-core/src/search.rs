//! Landmark filtering, nearest-neighbour lookup, and GeoJSON export.

use serde::{Deserialize, Serialize};

use crate::coords::{distance, LatLon, WorldPoint};
use crate::landmarks::{IrlStatus, Landmark};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LandmarkFilter {
    pub text: Option<String>,
    pub tags: Option<Vec<String>>,
    pub statuses: Option<Vec<IrlStatus>>,
    pub districts: Option<Vec<String>>,
    pub positioned_only: bool,
}

fn fold_latin_base(ch: char) -> Option<char> {
    match ch {
        'À'..='Å' | 'à'..='å' | 'Ā'..='ą' => Some('a'),
        'Ç' | 'ç' | 'Ć'..='č' => Some('c'),
        'Ď'..='đ' => Some('d'),
        'È'..='Ë' | 'è'..='ë' | 'Ē'..='ě' => Some('e'),
        'Ĝ'..='ģ' => Some('g'),
        'Ĥ'..='ħ' => Some('h'),
        'Ì'..='Ï' | 'ì'..='ï' | 'Ĩ'..='ı' => Some('i'),
        'Ĵ' | 'ĵ' => Some('j'),
        'Ķ'..='ĸ' => Some('k'),
        'Ĺ'..='ł' => Some('l'),
        'Ñ' | 'ñ' | 'Ń'..='ŋ' => Some('n'),
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' | 'Ō'..='ő' => Some('o'),
        'Ŕ'..='ř' => Some('r'),
        'Ś'..='š' => Some('s'),
        'Ţ'..='ŧ' => Some('t'),
        'Ù'..='Ü' | 'ù'..='ü' | 'Ũ'..='ų' => Some('u'),
        'Ŵ' | 'ŵ' => Some('w'),
        'Ý' | 'ý' | 'ÿ' | 'Ŷ'..='Ÿ' => Some('y'),
        'Ź'..='ž' => Some('z'),
        _ => None,
    }
}

fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ('\u{0300}'..='\u{036f}').contains(&ch) {
            continue;
        }
        if let Some(base) = fold_latin_base(ch) {
            out.push(base);
        } else {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
        }
    }
    out
}

pub fn filter_landmarks(list: &[Landmark], f: &LandmarkFilter) -> Vec<Landmark> {
    let terms: Vec<String> = f
        .text
        .as_deref()
        .map(|t| fold(t).split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();

    list.iter()
        .filter(|l| {
            if f.positioned_only && l.ig.is_none() {
                return false;
            }
            if let Some(tags) = f.tags.as_deref() {
                if !tags.is_empty() && !l.tags.iter().any(|t| tags.contains(t)) {
                    return false;
                }
            }
            if let Some(statuses) = f.statuses.as_deref() {
                if !statuses.is_empty() && !statuses.contains(&l.irl_status) {
                    return false;
                }
            }
            if let Some(districts) = f.districts.as_deref() {
                if !districts.is_empty() {
                    let d = l.district.as_deref().unwrap_or("");
                    if !districts.iter().any(|cand| cand == d) {
                        return false;
                    }
                }
            }
            if !terms.is_empty() {
                let hay = fold(&format!(
                    "{}\n{}\n{}\n{}",
                    l.id,
                    l.ig_address,
                    l.irl_address,
                    l.tags.join(" ")
                ));
                if !terms.iter().all(|t| hay.contains(t.as_str())) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearestLandmark {
    pub landmark: Landmark,
    pub distance: f64,
}

pub fn nearest_landmarks(list: &[Landmark], p: WorldPoint, n: usize) -> Vec<NearestLandmark> {
    let mut items: Vec<NearestLandmark> = list
        .iter()
        .filter_map(|l| {
            l.ig.map(|ig| NearestLandmark {
                landmark: l.clone(),
                distance: distance(ig, p),
            })
        })
        .collect();
    items.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    items.truncate(n);
    items
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoJsonCrsProperties {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoJsonCrs {
    #[serde(rename = "type")]
    pub crs_type: String,
    pub properties: GeoJsonCrsProperties,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoJsonPointGeometry {
    #[serde(rename = "type")]
    pub geometry_type: String,
    pub coordinates: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoJsonLandmarkProperties {
    pub name: String,
    pub ig_address: String,
    pub district: Option<String>,
    pub irl_address: String,
    pub irl_status: IrlStatus,
    pub irl: Option<LatLon>,
    pub tags: Vec<String>,
    pub color: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoJsonFeature {
    #[serde(rename = "type")]
    pub feature_type: String,
    pub id: String,
    pub geometry: GeoJsonPointGeometry,
    pub properties: GeoJsonLandmarkProperties,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoJsonFeatureCollection {
    #[serde(rename = "type")]
    pub collection_type: String,
    pub crs: GeoJsonCrs,
    pub features: Vec<GeoJsonFeature>,
}

/// GeoJSON (non-geographic CRS: coordinates are Leonida world metres).
pub fn to_geojson(list: &[Landmark]) -> GeoJsonFeatureCollection {
    let features = list
        .iter()
        .filter_map(|l| {
            let ig = l.ig?;
            Some(GeoJsonFeature {
                feature_type: "Feature".to_string(),
                id: l.id.clone(),
                geometry: GeoJsonPointGeometry {
                    geometry_type: "Point".to_string(),
                    coordinates: [ig.x, ig.y],
                },
                properties: GeoJsonLandmarkProperties {
                    name: l.name.clone(),
                    ig_address: l.ig_address.clone(),
                    district: l.district.clone(),
                    irl_address: l.irl_address.clone(),
                    irl_status: l.irl_status,
                    irl: l.irl,
                    tags: l.tags.clone(),
                    color: format!("#{}", l.color),
                    source: l.source.clone(),
                },
            })
        })
        .collect();

    GeoJsonFeatureCollection {
        collection_type: "FeatureCollection".to_string(),
        crs: GeoJsonCrs {
            crs_type: "name".to_string(),
            properties: GeoJsonCrsProperties {
                name: "urn:leonida:world-metres".to_string(),
            },
        },
        features,
    }
}
