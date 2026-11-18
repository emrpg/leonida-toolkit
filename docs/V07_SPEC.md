# Leonida Toolkit v0.7 架构契约：RAGE NativeDB & ScriptHookV ABI 工作台（`apps/natives`）

基于 `/root/orca-workspaces/gta-repos/citizenfx-natives`（6,000+ Native 文档）与 `/root/wsl-demo/rea-reconstructions/closed-source-code/scripthookv_fiber_tls_reconstructed.cpp`，构建独立开发者工作台（挂载于 `http://localhost:8080/natives/`）。

## 1. 核心模型与代码生成（`crates/leonida-core/src/natives.rs`）

- `NativeParam { name, param_type }`
- `NativeEntry { namespace, name, hash, jhash?, params, return_type, description }`
- `NativeNamespaceStat { namespace, count }`
- `filter_natives(entries, q, namespace, limit)` / `native_stats(entries)`
- `generate_shv_stub(entry, lang: "cpp" | "csharp" | "rust") -> String`（生成 ScriptHookV `invoke` / SHVDN `Function.Call` / Rust FFI 调用代码片段）

## 2. CLI 数据索引与 WASM（`crates/leonida-cli`, `crates/leonida-wasm`）

- `leonida natives build [--dir /root/orca-workspaces/gta-repos/citizenfx-natives] [--out apps/natives/public/data/natives.json]`：解析 Markdown `.md` 生成紧凑 `natives.json`
- `leonida natives search <q>` / `leonida natives stats`
- `leonida-wasm` 导出 `generate_shv_stub` 并同步构建至 `apps/natives/src/wasm/`

## 3. 服务端路由与书签持久化（`crates/leonida-server`）

- 静态托管 `apps/natives/dist` 到 `/natives`（`GET /natives` 与 `/natives/*`）
- `GET /api/natives?q=&ns=&limit=100` & `GET /api/natives/stats`
- `GET/POST/DELETE /api/natives/bookmarks`（SQLite 持久化收藏 Native 与开发备注）

## 4. 独立前端工作台（`apps/natives`，`base: '/natives/'`）

- 左侧命名空间导航（40+ 命名空间 + 收藏夹）
- 顶部毫秒级检索（按函数名 / 64-bit 哈希 / 描述）
- 右侧详情与代码生成器：参数签名、C++ ScriptHookV / C# SHVDN / Rust FFI 一键复制、SQLite 书签、以及 ScriptHookV `scrThread` TLS / 纤程调度架构速查面板
