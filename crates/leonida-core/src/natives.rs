//! RAGE NativeDB models, filtering, namespace statistics, and ScriptHookV stub codegen (v0.7).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Parameter signature of a RAGE native function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeParam {
    pub name: String,
    pub param_type: String,
}

/// Metadata for a single RAGE native function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeEntry {
    pub namespace: String,
    pub name: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jhash: Option<String>,
    #[serde(default)]
    pub params: Vec<NativeParam>,
    pub return_type: String,
    #[serde(default)]
    pub description: String,
}

/// Entry count for a single native namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeNamespaceStat {
    pub namespace: String,
    pub count: usize,
}

/// Filter native entries by optional query string, optional namespace, and maximum result limit.
pub fn filter_natives(
    entries: &[NativeEntry],
    q: Option<&str>,
    namespace: Option<&str>,
    limit: usize,
) -> Vec<NativeEntry> {
    if limit == 0 {
        return Vec::new();
    }

    let ns_filter = namespace.map(str::trim).filter(|s| !s.is_empty());
    let terms: Vec<String> = q
        .map(|s| {
            s.to_lowercase()
                .split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for entry in entries {
        if let Some(ns) = ns_filter {
            if !entry.namespace.eq_ignore_ascii_case(ns) {
                continue;
            }
        }
        if !terms.is_empty() {
            let hay = format!(
                "{}\n{}\n{}\n{}\n{}",
                entry.namespace,
                entry.name,
                entry.hash,
                entry.jhash.as_deref().unwrap_or(""),
                entry.description
            )
            .to_lowercase();
            if !terms.iter().all(|t| hay.contains(t.as_str())) {
                continue;
            }
        }
        out.push(entry.clone());
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// Compute per-namespace counts sorted by descending count, then ascending namespace name.
pub fn native_stats(entries: &[NativeEntry]) -> Vec<NativeNamespaceStat> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.namespace.clone()).or_insert(0) += 1;
    }

    let mut stats: Vec<NativeNamespaceStat> = counts
        .into_iter()
        .map(|(namespace, count)| NativeNamespaceStat { namespace, count })
        .collect();
    stats.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.namespace.cmp(&b.namespace))
    });
    stats
}

fn is_void_type(ty: &str) -> bool {
    ty.trim().eq_ignore_ascii_case("void") || ty.trim().is_empty()
}

fn map_csharp_type(ty: &str) -> &str {
    let t = ty.trim();
    if t.eq_ignore_ascii_case("BOOL") {
        "bool"
    } else if t.eq_ignore_ascii_case("float") {
        "float"
    } else if t.eq_ignore_ascii_case("const char*") || t.eq_ignore_ascii_case("char*") {
        "string"
    } else if t.eq_ignore_ascii_case("Vector3") {
        "Vector3"
    } else if t.eq_ignore_ascii_case("Hash") {
        "uint"
    } else if is_void_type(t) {
        "void"
    } else {
        "int"
    }
}

fn map_rust_type(ty: &str) -> &str {
    let t = ty.trim();
    if t.eq_ignore_ascii_case("BOOL") {
        "bool"
    } else if t.eq_ignore_ascii_case("float") {
        "f32"
    } else if t.eq_ignore_ascii_case("const char*") || t.eq_ignore_ascii_case("char*") {
        "&str"
    } else if t.eq_ignore_ascii_case("Vector3") {
        "[f32; 3]"
    } else if t.eq_ignore_ascii_case("Hash") {
        "u32"
    } else if is_void_type(t) {
        "()"
    } else if t.contains('*') {
        "usize"
    } else {
        "i32"
    }
}

fn format_hex_literal(hash: &str) -> String {
    let trimmed = hash.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        trimmed.to_string()
    } else {
        format!("0x{trimmed}")
    }
}

/// Generate a ScriptHookV (`cpp`), ScriptHookVDotNet (`csharp`), or Rust FFI (`rust`) call stub.
pub fn generate_shv_stub(entry: &NativeEntry, lang: &str) -> String {
    let hash_lit = format_hex_literal(&entry.hash);
    match lang.trim().to_ascii_lowercase().as_str() {
        "cpp" | "c++" => {
            let ret = if is_void_type(&entry.return_type) {
                "void"
            } else {
                entry.return_type.trim()
            };
            let params: Vec<String> = entry
                .params
                .iter()
                .map(|p| format!("{} {}", p.param_type.trim(), p.name.trim()))
                .collect();
            let mut invoke_args = Vec::with_capacity(entry.params.len() + 1);
            invoke_args.push(hash_lit);
            invoke_args.extend(entry.params.iter().map(|p| p.name.trim().to_string()));
            let call = if is_void_type(&entry.return_type) {
                format!("invoke<Void>({});", invoke_args.join(", "))
            } else {
                format!("return invoke<{ret}>({});", invoke_args.join(", "))
            };
            format!(
                "// {}::{}\nstatic inline {} {}({}) {{ {} }}",
                entry.namespace,
                entry.name,
                ret,
                entry.name,
                params.join(", "),
                call
            )
        }
        "csharp" | "cs" | "c#" => {
            let ret = map_csharp_type(&entry.return_type);
            let params: Vec<String> = entry
                .params
                .iter()
                .map(|p| format!("{} {}", map_csharp_type(&p.param_type), p.name.trim()))
                .collect();
            let mut call_args = Vec::with_capacity(entry.params.len() + 1);
            call_args.push(format!("(Hash){hash_lit}"));
            call_args.extend(entry.params.iter().map(|p| p.name.trim().to_string()));
            let body = if ret == "void" {
                format!("Function.Call({});", call_args.join(", "))
            } else {
                format!("return Function.Call<{ret}>({});", call_args.join(", "))
            };
            format!(
                "// {}::{}\npublic static {} {}({}) {{ {} }}",
                entry.namespace,
                entry.name,
                ret,
                entry.name,
                params.join(", "),
                body
            )
        }
        "rust" | "rs" => {
            let fn_name = entry.name.to_ascii_lowercase();
            let ret = map_rust_type(&entry.return_type);
            let params: Vec<String> = entry
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name.trim(), map_rust_type(&p.param_type)))
                .collect();
            let args: Vec<String> = entry
                .params
                .iter()
                .map(|p| format!("NativeArg::from({})", p.name.trim()))
                .collect();
            let ret_sig = if ret == "()" {
                String::new()
            } else {
                format!(" -> {ret}")
            };
            format!(
                "// {}::{}\npub fn {}({}){} {{\n    native_invoke({}, &[{}])\n}}",
                entry.namespace,
                entry.name,
                fn_name,
                params.join(", "),
                ret_sig,
                hash_lit,
                args.join(", ")
            )
        }
        other => format!("// Unsupported target language: {other}"),
    }
}
