# Leonida Toolkit v0.6 架构契约：LeonidaOS 伴侣手机 App（`apps/phone`）

参考 `/root/wsl-demo/gta-peripherals/tools-and-apps/npwd` 与 `docs/ROADMAP.md` P2，打造独立移动端 PWA 伴侣应用（挂载于 `http://localhost:8080/phone/`）。

## 1. 数据模型（`crates/leonida-core/src/phone.rs`）

- `PhoneContact { id, name, role, district, avatar_color, bio }`
- `PhoneMessage { id, contact_id, sender, text, timestamp, unread }`
- `SnapmaticPhoto { id, title, district, x, y, filter, caption, image_data_url, created_at }`
- `PhoneNote { id, title, body, x?, y?, updated_at }`

## 2. 服务端路由与持久化（`crates/leonida-server`）

- 静态托管 `apps/phone/dist` 到 `/phone`（`GET /phone` 与 `/phone/*`）
- SQLite 持久化 API：
  - `GET /api/phone/contacts`（内置初始联系人）
  - `GET/POST /api/phone/messages`（按 `contact_id` 查询与发送消息）
  - `GET/POST/DELETE /api/phone/notes`（关联地图坐标的随身笔记）
  - `GET/POST/DELETE /api/phone/photos`（Snapmatic 地理标签相册）
- 复用现有 `/api/companion/*`、`/api/landmarks`、`/api/route`、`/api/sol/challenge/submit`

## 3. WASM 与 CLI（`crates/leonida-wasm`, `crates/leonida-cli`）

- `wasm-pack build --target web --out-dir ../../apps/phone/src/wasm` 同步输出 WASM 到 `apps/phone/src/wasm/`
- `leonida-cli` 新增 `phone stats` 统计/校验子命令

## 4. 独立手机前端（`apps/phone`）

- 移动端优先 PWA（`manifest.webmanifest` + `sw.js`），桌面端显示 LeonidaOS 手机外壳，手机端全屏自适应
- 6 大手机 App（主屏图标网格 + 底部 Dock）：
  1. **Radar 随身地图**（Leaflet + 路线规划 + 一键插旗）
  2. **Checklist 收集品清单**（对接 `/api/companion/checklist`）
  3. **Messages 线人消息**（NPWD 风格联系人与聊天，对接 `/api/phone/messages`）
  4. **Snapmatic 相册**（上传/拍照 + 滤镜 + 地理坐标绑定，对接 `/api/phone/photos`）
  5. **Notes 随身笔记**（对接 `/api/phone/notes`）
  6. **Challenge 猜图快玩**（对接 WASM 计分与 `/api/sol/challenge/submit`）
