# Leonida Toolkit 研发路线图（GTA VI 社区工具）

> 负责人交接日期：2026-11-18 · 输入：`/root/wsl-demo/reports/00–12`、REA 重构源码、本地参考仓库

## 1. 产品定位

基于调研结论（报告 00）：GTA VI 目前仅有主机版、包体加密，**L1–L4 二进制层零暴露**，社区生态集中在 **L0 外部观测与摄影测量层**。
因此工具研发按"可及性"分层推进：先做今天就有数据、有用户的 L0 工具（地图 / 地标 / 摄影测量 / 伴侣 App），
RAGE 资产/存档/配置类工具以 **GTA V 已公开格式** 为验证平台，待 GTA VI PC 版发布后再适配。

## 2. 模块规划

| 阶段 | 模块 | 内容 | 复用资产 | 状态 |
| :-- | :-- | :-- | :-- | :-- |
| P0 | `packages/core` | 世界坐标系、瓦片金字塔、地标数据适配、测距/面积、方位线三角定位、现实→游戏坐标预测、GeoJSON 导出 | gta6map `tiles.py`、gtamaplib、atlas 坐标重构（报告 11） | ✅ v0.1 |
| P0 | `apps/map` 2D 互动地图 | Leaflet + 自定义 CRS；底图切换/叠加层；731 地标搜索与筛选；3D 地标层；测距、三角定位、IRL→IG 预测；个人标记（本地存储、导入导出）；URL 状态 | gta6map 瓦片（162 MB, Public Domain） | ✅ v0.1 |
| P1 | `apps/tracker` 摄影测量工作台 | 截图导入 → 标注消失线/地标像素 → 相机标定（FOV / yaw / pitch / 高度）→ 多相机三角测量；移植 gtamaplib `Camera.find_camera` 参数扫描为 TS/WebWorker | gtamaplib（3874 行）、gtamaplib-vc 优化器与 UI | 规划 |
| P1 | 3D 视图 | three.js 显示 gtamaplib-vc 107 个带高度地标 + 底图贴地；第一人称街景模式 | gtamaplib-vc 零依赖 WebGL 引擎、atlas SVG/3D 变换（报告 11） | 规划 |
| P2 | `apps/companion` 伴侣 PWA | 离线地图、收集品/任务清单、进度同步（本地优先 + 可选导出），手机端交互参考 npwd | npwd、live_map / webmap 架构（报告 06/07） | 规划 |
| P2 | 数据管线 | 定时同步上游社区数据集（gta6map、gtadb CC BY 4.0、gtamaplib-vc），版本固定 + SHA-256 校验 + 变更日志 | gtadb 溯源常量（`leonida-source-coords/gtadb.ts`） | 规划 |
| P3 | `tools/rage` 资产/配置工具 | 以 GTA V 为验证平台：RPF7 目录浏览（仅 OPEN/未加密归档或用户自备密钥）、RSC7 页大小解码、handling.meta 编辑器、存档/照片元数据查看 | `rpf_rsc7_crypto_reconstructed.cs`（报告 12）、CodeWalker、GTAVHandlingEditor、gta5view | 规划 · 待 GTA VI PC 版 |

## 3. 合规边界（所有模块强制执行）

- **不分发** Rockstar 密钥、游戏二进制、受版权保护的游戏资产；RPF 解密只对用户自有拷贝、由用户自行提供密钥。
- **不做** 在线作弊、反作弊绕过、主机破解/DRM 规避相关功能。ScriptHookV 纤程/TLS 重构（报告 09/12）仅作为单机模组生态的架构参考。
- **不调用** 第三方站点的私有/管理端点（如 stateofleonida `admin_*` 接口）；不抓取未授权数据集。仅使用明确授权的数据：gta6map / gtamaplib / gtamaplib-vc（Public Domain）、gtadb（CC BY 4.0，需署名）。
- gta6-leonida-atlas 为 **AGPL-3.0**：不直接拷贝其源码；仅复用公开的坐标约定事实（已在 `core/coords.ts` 中独立实现）。
- 所有界面标注"社区估算，非官方地图 / 非 Rockstar 官方产品"。

## 4. 近期里程碑

1. **M1（本周）**：P0 完成 ✅ → 部署静态站点（瓦片走 CDN/对象存储，不进 git）。
2. **M2**：摄影测量工作台 MVP（单相机标定 + 双相机三角测量，与 gtamaplib Python 结果回归对齐）。
3. **M3**：3D 视图 + gtadb 数据接入（含署名）。
4. **M4**：伴侣 PWA 离线版。
5. **GTA VI PC 版发布后**：重新评估 L1/L2 可及性，启动 RPF8 / 新 RSC 格式调研与 `tools/rage` 适配。

## 5. 关键指标（v0.1 基线）

- 地标 731（已定位 696，现实确认 555+）；3D 地标 107；瓦片集 8，瓦片 4 611。
- 现实→游戏坐标预测（留一法交叉验证，555 个锚点）：**中位误差 122 m，P90 876 m**。
