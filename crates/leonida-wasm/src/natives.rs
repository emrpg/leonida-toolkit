use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeParam {
    pub name: String,
    pub param_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeEntry {
    pub namespace: String,
    pub name: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jhash: Option<String>,
    pub params: Vec<NativeParam>,
    pub return_type: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeNamespaceStat {
    pub namespace: String,
    pub count: usize,
}

pub fn filter_natives<'a>(
    entries: &'a [NativeEntry],
    q: Option<&str>,
    namespace: Option<&str>,
    limit: usize,
) -> Vec<&'a NativeEntry> {
    let q_lower = q
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);
    let ns_filter = namespace.map(str::trim).filter(|s| !s.is_empty());

    entries
        .iter()
        .filter(|e| {
            if let Some(ns) = ns_filter {
                if !e.namespace.eq_ignore_ascii_case(ns) {
                    return false;
                }
            }
            if let Some(ref query) = q_lower {
                let in_name = e.name.to_lowercase().contains(query);
                let in_hash = e.hash.to_lowercase().contains(query);
                let in_jhash = e
                    .jhash
                    .as_deref()
                    .is_some_and(|j| j.to_lowercase().contains(query));
                let in_desc = e.description.to_lowercase().contains(query);
                if !(in_name || in_hash || in_jhash || in_desc) {
                    return false;
                }
            }
            true
        })
        .take(limit)
        .collect()
}

pub fn native_stats(entries: &[NativeEntry]) -> Vec<NativeNamespaceStat> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in entries {
        *counts.entry(e.namespace.clone()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(namespace, count)| NativeNamespaceStat { namespace, count })
        .collect()
}

fn map_cs_type(c_type: &str) -> &str {
    match c_type.trim() {
        "void" => "void",
        "BOOL" | "bool" => "bool",
        "int" | "Any" | "Player" | "Ped" | "Vehicle" | "Entity" | "Object" | "Cam" | "Blip"
        | "Hash" | "ScrHandle" | "FireId" | "Interior" | "Pickup" => "int",
        "float" => "float",
        "char*" | "const char*" => "string",
        "Vector3" => "Vector3",
        _ => "ulong",
    }
}

fn map_rust_type(c_type: &str) -> &str {
    match c_type.trim() {
        "void" => "()",
        "BOOL" | "bool" => "bool",
        "int" | "Any" | "Player" | "Ped" | "Vehicle" | "Entity" | "Object" | "Cam" | "Blip"
        | "Hash" | "ScrHandle" | "FireId" | "Interior" | "Pickup" => "i32",
        "float" => "f32",
        "char*" | "const char*" => "*const c_char",
        "Vector3" => "[f32; 3]",
        _ => "u64",
    }
}

pub fn generate_shv_stub(entry: &NativeEntry, lang: &str) -> Result<String, String> {
    let args_decl: Vec<String> = entry
        .params
        .iter()
        .map(|p| format!("{} {}", p.param_type, p.name))
        .collect();
    let arg_names: Vec<&str> = entry.params.iter().map(|p| p.name.as_str()).collect();

    match lang.to_ascii_lowercase().as_str() {
        "cpp" | "c++" => {
            let call_args = if arg_names.is_empty() {
                String::new()
            } else {
                format!(", {}", arg_names.join(", "))
            };
            Ok(format!(
                "// {}::{}\nstatic inline {} {}({}) {{\n    return invoke<{}>({}{});\n}}",
                entry.namespace,
                entry.name,
                entry.return_type,
                entry.name,
                args_decl.join(", "),
                entry.return_type,
                entry.hash,
                call_args,
            ))
        }
        "csharp" | "cs" | "c#" => {
            let cs_ret = map_cs_type(&entry.return_type);
            let cs_params: Vec<String> = entry
                .params
                .iter()
                .map(|p| format!("{} {}", map_cs_type(&p.param_type), p.name))
                .collect();
            let call_args = if arg_names.is_empty() {
                String::new()
            } else {
                format!(", {}", arg_names.join(", "))
            };
            let body = if cs_ret == "void" {
                format!("Function.Call((Hash){}{});", entry.hash, call_args)
            } else {
                format!(
                    "return Function.Call<{}>((Hash){}{});",
                    cs_ret, entry.hash, call_args
                )
            };
            Ok(format!(
                "// {}::{}\npublic static {} {}({}) {{\n    {}\n}}",
                entry.namespace,
                entry.name,
                cs_ret,
                entry.name,
                cs_params.join(", "),
                body,
            ))
        }
        "rust" | "rs" => {
            let rs_ret = map_rust_type(&entry.return_type);
            let rs_params: Vec<String> = entry
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, map_rust_type(&p.param_type)))
                .collect();
            let args_slice = if arg_names.is_empty() {
                "&[]".to_string()
            } else {
                let casts: Vec<String> = arg_names.iter().map(|n| format!("{n} as u64")).collect();
                format!("&[{}]", casts.join(", "))
            };
            Ok(format!(
                "// {}::{}\npub unsafe fn {}({}) -> {} {{\n    shv_invoke::<{}>({}, {})\n}}",
                entry.namespace,
                entry.name,
                entry.name.to_ascii_lowercase(),
                rs_params.join(", "),
                rs_ret,
                rs_ret,
                entry.hash,
                args_slice,
            ))
        }
        other => Err(format!(
            "unsupported language '{other}': expected cpp, csharp, or rust"
        )),
    }
}
