# State of Leonida 完整闭源复刻架构契约（v0.3）

基于 `/root/wsl-demo/rea-reconstructions/web-and-js/reconstructions/stateofleonida/StateOfLeonida_ApiAndMapClient.ts` 与 `01_stateofleonida_endpoints.csv`、`02_stateofleonida_storage_keys.csv` 构建完整 State of Leonida 平台（Rust + WASM + Axum + SQLite + TypeScript）。

## 1. 数据集契约（`apps/map/public/data/`）

由 `leonida-cli`（`build-sol-data`）生成至 `apps/map/public/data/`：
- `markers.json` / `trailer1.json` / `trailer2.json` / `trailer3.json` / `leaks.json` / `screenshots.json` / `el_screenshots.json`
  - 统一 `SolEntry` 结构：`{ id, title, category, dataset, x, y, z?, district?, status?, description?, timestampSec?, mediaUrl?, realWorld? }`
- `news.json` / `database_changelog.json` / `content.json` / `credits.json` / `supporters.json` / `backgrounds.json` / `lang.json` / `seo.json`

## 2. 服务端接口契约（`crates/leonida-server`）

同时支持原版 `auth/api.php?action=...` 与 `/api/sol/...`：
- `GET /auth/api.php?action=access_config`
- `POST /auth/api.php?action=access_verify`
- `GET /auth/api.php?action=gtadb_source_public`
- `GET /auth/api.php?action=site_settings_public`
- `GET /auth/api.php?action=ranks`
- `GET /auth/api.php?action=notifications` / `POST ?action=notifications_read`
- `GET /auth/api.php?action=marker_list`
- `POST /auth/api.php?action=marker_save` / `marker_update` / `marker_delete`
- `GET /auth/api.php?action=game_stats&game=screenshot_challenge`
- `GET /auth/api.php?action=leaderboard&game=screenshot_challenge&limit=10`
- `POST /api/sol/challenge/submit`（提交猜图挑战成绩并写入 SQLite 排行榜）
- `GET /get-media-files.php?action=tabs` & `?type=trailer1|trailer2|trailer3|leaks|screenshots`
- `GET /data/*` 静态提供 `apps/map/public/data/*`

## 3. 核心算法（`crates/leonida-core` & `crates/leonida-wasm`）

- `SolEntry` / `SolDatasetStats` / `ChallengeScoreResult`
- `score_screenshot_challenge(target_x, target_y, guess_x, guess_y) -> ChallengeScoreResult { distance_m, score, grade }`
  - `distance_m = hypot(dx, dy)`
  - `score = round(5000 * exp(-distance_m / 1200))`（0–5000 分），`grade`: S / A / B / C / D
