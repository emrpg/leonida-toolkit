# Leonida Toolkit · GTA VI 社区工具集

GTA VI（Leonida 州）社区工具：互动地图、地标追踪、摄影测量与伴侣应用。路线图见 [docs/ROADMAP.md](docs/ROADMAP.md)。

> 非 Rockstar Games / Take-Two 官方产品；不含任何游戏二进制或受版权保护的游戏资产。

## 结构

```
packages/core   @leonida/core  坐标系 / 瓦片金字塔 / 地标适配 / 几何与三角定位 / IRL→IG 预测（含 vitest 测试）
apps/map        @leonida/map   Leaflet 2D 互动地图（Vite + TypeScript）
data/raw        上游社区数据快照（Public Domain）
scripts         数据构建与资产链接脚本
```

## 快速开始

```bash
npm install
npm run assets:link   # 链接 gta6map 瓦片（默认 /root/wsl-demo/...，可用 GTA6MAP_DIR 覆盖）
npm run data:build    # 生成 apps/map/public/data（地标 + 瓦片清单）
npm test              # core 单元测试 + 真实数据回归
npm run dev           # http://localhost:5173
```

## 地图功能（v0.1）

- 社区底图（dupzor / yanis）与叠加层切换，0–6 级瓦片（level 5 = 1 m/px）
- 731 个地标：全文搜索、标签与现实确认状态筛选、详情弹窗（现实原型 + Google 地图链接 + 附近地标）
- gtamaplib-vc 3D 三角测量地标层（含高度）
- 工具：测距 / 面积、方位线最小二乘三角定位、现实经纬度 → 游戏坐标预测
- 个人标记（右键添加，本地存储，JSON 导入导出）、GeoJSON 导出、URL 视图状态 `#x,y,zoom`

## 数据来源与许可

- [gta6map.github.io](https://github.com/gta6map/gta6map.github.io) — 地标与瓦片，Public Domain
- [gtamaplib-vc](https://github.com/rolux/gtamaplib-vc) — 3D 地标，Public Domain
