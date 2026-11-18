# State of Leonida 1:1 100% 完整复刻架构契约（v0.8）

对齐 `/root/wsl-demo/rea-reconstructions/web-and-js/reconstructions/16_stateofleonida_string_pool.csv` 与 `01_stateofleonida_endpoints.csv`、`02_stateofleonida_storage_keys.csv`，补齐 `https://map.stateofleonida.net/` 全量特性。

## 1. 核心算法与模型（`crates/leonida-core` & `crates/leonida-wasm`）

- **热力图网格聚合（`compute_heatmap_cells`）**：按世界坐标网格统计地标密度与归一化权重（0.0..1.0），供 Leaflet Canvas 热力图渲染。
- **Marker Motion 路径插值（`interpolate_motion_path`）**：给定路径点列表与归一化进度 `t ∈ [0, 1]`，返回平滑插值坐标与朝向角（bearing）。
- **扩展模型**：`MarkerComment { id, marker_id, author, body, created_at }`、`AdminDbOverview`、`WikiCacheEntry`。

## 2. 服务端完整闭环（`crates/leonida-server`）

补全 `auth/api.php?action=...` 自托管 SQLite 接口：
- **讨论区**：`marker_discussion_list` / `marker_discussion_post` / `marker_discussion_stats`
- **Admin Studio**：`admin_stats`、`admin_db_overview`、`admin_db_export`、`admin_db_integrity`、`admin_news`、`admin_news_save`、`admin_site_settings`、`admin_site_settings_save`、`admin_credits`、`admin_ranks`、`admin_users`
- **数据接口**：`GET /data/wiki-cache.json`、`POST /save-changelog.php`

## 3. 前端 1:1 Map Studio 复刻（`apps/map`）

- **Preloader 开屏动画**（可通过 `isPreloaderStyle` 设置开关，支持一键跳过）
- **地图图层**：
  - `heatmapEnabled`：Canvas 热力图叠加层（调用 WASM `compute_heatmap_cells`）
  - `gridEnabled`：Leonida 1000m 世界坐标网格叠加层
  - `clusteringEnabled`：标记聚合与 `clusterCustomColors`
  - `markerMotionEnabled`：路径动画录制/播放（调用 WASM `interpolate_motion_path`）
- **Map Studio 主题引擎**（持久化到 `localStorage` 对应原版键）：
  - `accentColor`（Neon Pink / Vice Cyan / Sunset Gold / Emerald）、`panelStyle`（Glass / Solid / Minimal）、`listDensity`（Compact / Comfortable / Cozy）、`selectedBackgroundColor`
- **地标讨论区**：地标弹窗内支持查看/发表评论（SQLite 持久化）与 Wiki 摘要卡片
- **Admin Studio & Credits/Supporters**：顶部新增「管理台 Admin」Tab（数据库概览/完整性检查/全库导出/在线发新闻）与致谢/支持者面板
