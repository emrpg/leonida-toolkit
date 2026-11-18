# 团队任务分派（Rust 技术栈 · v0.2 · 4 人并行）

总负责人：coordinator。技术栈以 **Rust** 为主；浏览器 UI 继续用现有 Leaflet 前端（`apps/map`），但所有业务逻辑由 Rust（WASM + 服务端）提供。TS 版 `packages/core` 是**行为参考与测试基准**，Rust 达到同等能力后删除。

合规边界见 [ROADMAP.md §3](ROADMAP.md)，所有人必须遵守。

## 所有权（互不修改对方目录；根 `Cargo.toml` 由 coordinator 维护）

| 成员 | 终端 Handle | 拥有路径 | 交付物 |
| :-- | :-- | :-- | :-- |
| **core-rust** | `term_a58f28b9-c758-4470-a7d3-b3abc31dd7d6` | `crates/leonida-core` | 核心库（1:1 移植 + 单元测试 + 回归测试） |
| **cli-wasm-rust** | `term_f798683b-da92-4445-a6b3-b9f155cbaf08` | `crates/leonida-cli`, `crates/leonida-wasm` | CLI 工具 `leonida` + WASM 绑定（`apps/map/src/wasm/`） |
| **server-rust** | `term_00f26dff-492c-417a-98b7-c218a30a8d35` | `crates/leonida-server` | axum 服务端 + SQLite 标记持久化 + 集成测试 |
| **frontend-e2e** | `term_f7932ff6-7511-4d0c-9d3f-c6d0ceb14999` | `apps/map` | 前端接入 `leonida-wasm` 与 `/api/*` + Playwright 端到端验证 |

## 1. core-rust · 验收标准

1. `crates/leonida-core`：1:1 移植 `packages/core/src/*.ts`——
   - `coords`（`WorldPoint`/bearing/路径长度/面积/`AtlasFrame`）
   - `tiles`（`metres_per_pixel`、`world↔pixel`、`TileResolver` 复合→底图回退）
   - `landmarks`（gta6map 元组解析、gtamaplib-vc 解析、统计）
   - `geo`（haversine、局部投影、加权仿射拟合、`predict_ig` KNN 局部仿射、射线求交、N 射线最小二乘三角定位）
   - `search`（过滤、最近邻、GeoJSON）
2. 约束：serde 模型、`#![forbid(unsafe_code)]`、公开 API 无 panic（返回 `Option`/`Result`）。
3. 测试：移植 `packages/core/test/core.test.ts` 全部用例；真实数据留一法回归必须复现 **中位误差 ≈122 m（<400 m 断言）**。
4. 检查：`cargo fmt --check`、`cargo clippy -p leonida-core -- -D warnings`、`cargo test -p leonida-core` 全绿。

## 2. cli-wasm-rust · 验收标准

1. `crates/leonida-cli`（二进制名 `leonida`，使用 `clap`）：`build-data`（取代 `scripts/build-data.mjs`：输出 `apps/map/public/data/*` 与 `tile-manifest.json`）、`stats`、`search <q>`、`predict <lat> <lon>`、`triangulate x,y,bearing ...`、`export-geojson`。
2. `crates/leonida-wasm`（`wasm-bindgen` + `serde-wasm-bindgen`，执行 `wasm-pack build --target web --out-dir ../../apps/map/src/wasm`）：导出 `Landmarks` 句柄（`load`/`filter`/`nearest`/`stats`/`geojson`）、`predict_ig`、`triangulate`、`path_length`、`polygon_area`、`TileResolver`。
3. 先尽快设计并实现公开 API 签名（如 `leonida-core` 尚未提交，可先定义接口/占位并在 `leonida-core` 落地后立即对接），让 `frontend-e2e` 能同步接入。
4. 检查：`cargo fmt --check`、`cargo clippy -p leonida-cli -p leonida-wasm -- -D warnings`、`cargo test -p leonida-cli`、`wasm-pack build` 全绿。

## 3. server-rust · 验收标准

1. `crates/leonida-server`（`axum` + `tokio` + `tower-http` + `rusqlite` bundled）：
   - 静态托管 `apps/map/dist` 与 `/tiles`（瓦片目录可配置，`GTA6MAP_TILES`，默认 `apps/map/public/tiles`；带缓存头）。
   - REST：`GET /api/landmarks?q=&tags=&status=`、`GET /api/landmarks/:id`、`GET /api/nearest?x=&y=&n=`、`GET /api/predict?lat=&lon=`、`POST /api/triangulate`、`GET /api/stats`、`GET /api/tile-manifest`、`GET /api/geojson`。
   - 用户标记持久化：SQLite，`GET/POST/PUT/DELETE /api/markers`，导入导出 JSON（schema `leonida-user-markers-v1`，与前端现有格式兼容）。
2. 在 `leonida-core` 落地前可先用 trait/占位实现路由与 SQLite 层，落地后立即切换调用 `leonida-core`。
3. 集成测试（`tower::ServiceExt::oneshot`），`cargo fmt --check`、`cargo clippy -p leonida-server -- -D warnings`、`cargo test -p leonida-server` 全绿。

## 4. frontend-e2e · 验收标准

1. `apps/map`：去掉对 TS `@leonida/core` 的依赖，几何/瓦片/搜索/预测/三角定位计算改用 `leonida-wasm`（`apps/map/src/wasm/`），数据与用户标记走服务端 `/api/*`（离线或静态预览时优雅回退 `localStorage`）。保持现有全部功能与中文 UI。
2. 在 `leonida-wasm` 与 `leonida-server` 就绪前，可先整理 API 客户端与离线回退层并对接。
3. 端到端：`npm run build` 构建 `apps/map/dist` 后，`cargo run -p leonida-server` 访问 `http://localhost:8080` 验证地图、搜索、预测、三角定位、标记增删可用，并提供 Playwright 截图（可参考 `/tmp/pw/shot.mjs`）。

## 协作规则

- 小步提交，只 `git add` 自己拥有的路径（注意不要 `git add` 别人的目录或 `Cargo.lock` 冲突）；提交信息 `feat(core): …` / `feat(cli-wasm): …` / `feat(server): …` / `feat(map): …`。
- 完成后在终端输出清晰汇报：做了什么、测试结果、遗留问题。
