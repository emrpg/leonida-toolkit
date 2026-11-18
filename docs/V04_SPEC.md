# Leonida Toolkit v0.4 架构契约：3D 视界 + 摄影测量工作台 + 路线规划

基于 `/root/wsl-demo/rea-reconstructions/web-and-js/reconstructions/gta6-leonida-atlas/LeonidaAtlas_CoordsAndRoutes.ts` 与 `gtamaplib` / `gtamaplib-vc` 摄影测量模型，在现有 State of Leonida 平台上扩展三大能力：

## 1. 核心算法（`crates/leonida-core` & `crates/leonida-wasm`）

1. **Atlas 坐标变换**：
   - `CANONICAL_BOUNDS`: `west=-16000, east=4000, south=-8000, north=12000`
   - `world_to_svg(x, y) -> (x * 2.0, -y * 2.0)`；`svg_to_world(sx, sz) -> (sx / 2.0, -sz / 2.0)`
2. **摄影测量相机标定（`solve_camera_pose`）**：
   - 输入：`CameraObs[] { world_x, world_y, world_z, px, py }`、画幅 `(width, height)`、可选初始 `fov_deg`（默认 60°）
   - 输出：`CameraPose { x, y, z, yaw_deg, pitch_deg, fov_deg, rmse_px }`
   - 辅助：`frustum_polygon(x, y, yaw_deg, fov_deg, range_m) -> Vec<[f64; 2]>`（返回视锥扇区多边形用于地图绘制）
3. **路线规划（`plan_route`）**：
   - 基于地标/节点构建 KNN 连通图，使用 Dijkstra / A* 计算 `(x1, y1)` 到 `(x2, y2)` 最短路径
   - 输出：`RouteResult { waypoints: Vec<[f64; 2]>, distance_m: f64, drive_min: f64, fly_min: f64 }`

## 2. 服务端接口（`crates/leonida-server`）

- `POST /api/photogrammetry/solve`：求解相机位姿并返回 `CameraPose` + `frustum`
- `GET/POST/DELETE /api/photogrammetry/projects`：SQLite 持久化摄影测量标定工程
- `GET /api/route?x1=&y1=&x2=&y2=`：路线规划

## 3. 前端工作台（`apps/map`）

- 顶部导航新增 **3D 视界** 与 **摄影测量** Tab，地图工具栏增加 **路线规划** 模式：
  - **3D 视界**：WebGL / Three.js 渲染 Leonida 地面纹理与 `gtamaplib-vc` 107 个带 Z 轴高度的 3D 地标柱，支持轨道/第一人称漫游与点击定位。
  - **摄影测量工作台**：导入或选择截图 → 标注像素点并绑定地标 → 调用 WASM `solve_camera_pose` 解算相机位置与朝向 → 在地图绘制 FOV 视锥并支持保存到 SQLite。
  - **路线规划**：地图选起点/终点，调用 WASM/服务端 `plan_route` 绘制路径并显示里程与耗时。
