# Leonida Toolkit v0.5 架构契约：伴侣 PWA + RAGE 资产/配置工具

完成路线图 P2（伴侣 PWA 与收集品追踪）与 P3（RAGE 资产/配置工具，严格遵守合规边界：不内置 Rockstar 密钥、不绕过 DRM）。

## 1. 核心模块（`crates/leonida-core` & `crates/leonida-wasm`）

### `rage` 模块（`#![forbid(unsafe_code)]`）
- **RPF7 / RPF8 头部解析**：魔数 `0x52504637`（RPF7）/ `0x52504638`（RPF8）、`entry_count`、`names_length`、加密标识（`None`/`Open`/`Aes`/`Ng`）
- **RSC7 页重定位解码**：魔数 `0x37435352`（RSC7）、`system_flags`、`graphics_flags`、`compute_segment_size(flags)`
- **哈希与索引**：`joaat(name)`（Jenkins one-at-a-time 32-bit）与 `compute_ng_key_index(name, file_size)`
- **`handling.meta` 解析与导出**：纯 Rust 轻量 XML 解析/生成 `HandlingItem`（`handlingName`、`fMass`、`fInitialDragCoeff`、`fDriveBiasFront`、`nInitialDriveGears`、`fInitialDriveForce`、`fDriveMaxFlatVel`、`fBrakeForce`、`fTractionCurveMax`、`fSuspensionForce`）

### `companion` 模块
- `CollectibleItem { id, title, category, district, x, y, completed }`、`CompanionStats { total, completed, percent }`

## 2. 服务端接口（`crates/leonida-server`）

- `GET /api/companion/checklist`：返回收集品清单与 SQLite 完成状态
- `POST /api/companion/toggle`：`{ id, completed }` 切换完成状态
- `GET /api/companion/export` & `POST /api/companion/import`：导入导出进度 JSON
- `POST /api/rage/inspect-header`：输入 16 进制头部字节，返回 RPF7/RPF8/RSC7 解析结果
- `POST /api/rage/handling/parse` & `POST /api/rage/handling/export`：`handling.meta` XML ↔ JSON 互转

## 3. 前端工作台（`apps/map`）

- **PWA**：`public/manifest.webmanifest` + `public/sw.js` 离线缓存
- **伴侣清单（Companion）Tab**：分类清单、进度条、一键定位到地图、SQLite + `localStorage` 双写
- **RAGE 工具台（RAGE Tools）Tab**：`handling.meta` 可视化编辑器（含内置示例车辆）、RSC7/RPF7/RPF8 头部解码器、JOAAT 哈希计算器
