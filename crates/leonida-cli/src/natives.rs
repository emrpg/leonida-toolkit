use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct NativesArgs {
    #[command(subcommand)]
    pub command: NativesCommands,
}

#[derive(Subcommand, Debug)]
pub enum NativesCommands {
    /// Parse citizenfx-natives Markdown files into a compact natives.json index
    Build {
        /// Source directory containing namespace folders of .md files
        #[arg(
            long,
            default_value = "/root/orca-workspaces/gta-repos/citizenfx-natives"
        )]
        dir: PathBuf,
        /// Output JSON file path
        #[arg(long, default_value = "apps/natives/public/data/natives.json")]
        out: PathBuf,
    },
    /// Search natives.json by function name, hash, or description
    Search {
        /// Search query
        query: String,
        /// Filter by namespace (e.g. PLAYER)
        #[arg(long)]
        ns: Option<String>,
        /// Maximum results
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Path to natives.json
        #[arg(long, default_value = "apps/natives/public/data/natives.json")]
        data: PathBuf,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print per-namespace counts from natives.json
    Stats {
        /// Path to natives.json
        #[arg(long, default_value = "apps/natives/public/data/natives.json")]
        data: PathBuf,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

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

fn parse_params(raw_params: &str) -> Vec<NativeParam> {
    let trimmed = raw_params.trim();
    if trimmed.is_empty() || trimmed == "void" {
        return Vec::new();
    }
    trimmed
        .split(',')
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                return None;
            }
            let idx = p.rfind(char::is_whitespace)?;
            let mut param_type = p[..idx].trim().to_string();
            let mut name = p[idx + 1..].trim().to_string();
            while name.starts_with('*') {
                param_type.push('*');
                name = name.trim_start_matches('*').trim().to_string();
            }
            Some(NativeParam { name, param_type })
        })
        .collect()
}

pub fn parse_native_md(content: &str, fallback_ns: &str) -> Option<NativeEntry> {
    let mut namespace = fallback_ns.to_string();
    let mut name: Option<String> = None;
    let mut hash: Option<String> = None;
    let mut jhash: Option<String> = None;
    let mut return_type = String::from("void");
    let mut params = Vec::new();
    let mut desc_lines = Vec::new();

    let mut in_frontmatter = false;
    let mut frontmatter_done = false;
    let mut in_code = false;
    let mut code_done = false;
    let mut sig_lines = Vec::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim();

        if idx == 0 && trimmed == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" {
                in_frontmatter = false;
                frontmatter_done = true;
            } else if let Some(ns_val) = trimmed.strip_prefix("ns:") {
                let ns = ns_val.trim();
                if !ns.is_empty() {
                    namespace = ns.to_string();
                }
            }
            continue;
        }
        if !frontmatter_done {
            frontmatter_done = true;
        }

        if name.is_none() && trimmed.starts_with("## ") {
            name = Some(trimmed.trim_start_matches("## ").trim().to_string());
            continue;
        }

        if !code_done && trimmed.starts_with("```") {
            if in_code {
                in_code = false;
                code_done = true;
            } else {
                in_code = true;
            }
            continue;
        }

        if in_code {
            if let Some(comment) = trimmed.strip_prefix("//") {
                let mut tokens = comment.split_whitespace();
                if let Some(h) = tokens.next() {
                    if h.starts_with("0x") || h.starts_with("0X") {
                        hash = Some(h.to_string());
                    }
                }
                if let Some(jh) = tokens.next() {
                    if jh.starts_with("0x") || jh.starts_with("0X") {
                        jhash = Some(jh.to_string());
                    }
                }
            } else if !trimmed.is_empty() {
                sig_lines.push(trimmed.to_string());
            }
            continue;
        }

        if code_done {
            if trimmed.starts_with("## ") {
                break;
            }
            if !trimmed.is_empty() {
                desc_lines.push(trimmed.to_string());
            }
        }
    }

    let sig = sig_lines.join(" ");
    if let (Some(open_idx), Some(close_idx)) = (sig.find('('), sig.rfind(')')) {
        let before = sig[..open_idx].trim();
        let inside = &sig[open_idx + 1..close_idx];
        if let Some(sp) = before.rfind(char::is_whitespace) {
            return_type = before[..sp].trim().to_string();
            let sig_name = before[sp + 1..].trim().trim_start_matches('*');
            if name.is_none() && !sig_name.is_empty() {
                name = Some(sig_name.to_string());
            }
        }
        params = parse_params(inside);
    }

    let name = name?;
    let hash = hash?;
    Some(NativeEntry {
        namespace,
        name,
        hash,
        jhash,
        params,
        return_type,
        description: desc_lines.join(" "),
    })
}

pub fn build_natives_index(dir: &Path, out: &Path) -> Result<(usize, usize)> {
    let mut entries = Vec::new();
    for ns_entry in
        fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?
    {
        let ns_entry = ns_entry?;
        let ns_path = ns_entry.path();
        if !ns_path.is_dir() {
            continue;
        }
        let ns_name = ns_entry.file_name().to_string_lossy().into_owned();
        if ns_name.starts_with('.') {
            continue;
        }
        for file_entry in fs::read_dir(&ns_path)? {
            let file_entry = file_entry?;
            let fpath = file_entry.path();
            if fpath.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let content = fs::read_to_string(&fpath)?;
            if let Some(item) = parse_native_md(&content, &ns_name) {
                entries.push(item);
            }
        }
    }

    entries.sort_by(|a, b| {
        a.namespace
            .cmp(&b.namespace)
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut ns_set = BTreeMap::new();
    for e in &entries {
        *ns_set.entry(e.namespace.clone()).or_insert(0usize) += 1;
    }

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(out, serde_json::to_string(&entries)?)
        .with_context(|| format!("failed to write {}", out.display()))?;

    Ok((entries.len(), ns_set.len()))
}

pub fn run_natives(args: NativesArgs) -> Result<()> {
    match args.command {
        NativesCommands::Build { dir, out } => {
            let (count, namespaces) = build_natives_index(&dir, &out)?;
            println!(
                "[natives build] parsed={count} namespaces={namespaces} out={}",
                out.display()
            );
        }
        NativesCommands::Search {
            query,
            ns,
            limit,
            data,
            json,
        } => {
            let raw = fs::read_to_string(&data)
                .with_context(|| format!("failed to read {}", data.display()))?;
            let entries: Vec<NativeEntry> = serde_json::from_str(&raw)?;
            let q = query.trim().to_lowercase();
            let matches: Vec<&NativeEntry> = entries
                .iter()
                .filter(|e| {
                    if let Some(ref ns_filter) = ns {
                        if !e.namespace.eq_ignore_ascii_case(ns_filter) {
                            return false;
                        }
                    }
                    e.name.to_lowercase().contains(&q)
                        || e.hash.to_lowercase().contains(&q)
                        || e.jhash
                            .as_deref()
                            .is_some_and(|j| j.to_lowercase().contains(&q))
                        || e.description.to_lowercase().contains(&q)
                })
                .take(limit)
                .collect();

            if json {
                println!("{}", serde_json::to_string_pretty(&matches)?);
            } else if matches.is_empty() {
                println!("No matching natives.");
            } else {
                for e in matches {
                    println!(
                        "{:<14} {:<38} {} -> {}",
                        e.namespace, e.name, e.hash, e.return_type
                    );
                }
            }
        }
        NativesCommands::Stats { data, json } => {
            let raw = fs::read_to_string(&data)
                .with_context(|| format!("failed to read {}", data.display()))?;
            let entries: Vec<NativeEntry> = serde_json::from_str(&raw)?;
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for e in &entries {
                *counts.entry(e.namespace.clone()).or_insert(0) += 1;
            }
            let stats: Vec<NativeNamespaceStat> = counts
                .into_iter()
                .map(|(namespace, count)| NativeNamespaceStat { namespace, count })
                .collect();

            if json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!(
                    "Total natives: {} across {} namespaces",
                    entries.len(),
                    stats.len()
                );
                let mut sorted = stats;
                sorted.sort_by(|a, b| {
                    b.count
                        .cmp(&a.count)
                        .then_with(|| a.namespace.cmp(&b.namespace))
                });
                for s in sorted.into_iter().take(15) {
                    println!("  {:<18} {}", s.namespace, s.count);
                }
            }
        }
    }
    Ok(())
}
