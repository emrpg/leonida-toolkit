use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use leonida_core::{
    build_irl_to_ig_model, filter_landmarks, id_sort_key, landmark_stats, nearest_landmarks,
    parse_gta6map_tuple, predict_ig, to_geojson, triangulate, Anchor, Gta6MapTuple, IrlStatus,
    IrlToIgModel, Landmark, LandmarkFilter, LandmarkStats, LatLon, NearestLandmark, Prediction,
    Ray2D, Triangulation, WorldPoint,
};

pub fn parse_irl_status(s: &str) -> Option<IrlStatus> {
    match s.trim().to_ascii_lowercase().as_str() {
        "confirmed" => Some(IrlStatus::Confirmed),
        "unconfirmed" => Some(IrlStatus::Unconfirmed),
        "unknown" => Some(IrlStatus::Unknown),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraObs {
    #[serde(alias = "world_x")]
    pub world_x: f64,
    #[serde(alias = "world_y")]
    pub world_y: f64,
    #[serde(alias = "world_z")]
    pub world_z: f64,
    pub px: f64,
    pub py: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraPose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub fov_deg: f64,
    pub rmse_px: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteResult {
    pub waypoints: Vec<[f64; 2]>,
    pub distance_m: f64,
    pub drive_min: f64,
    pub fly_min: f64,
}

/// Core computation boundary backed by `leonida_core`.
pub trait LandmarkEngine: Send + Sync {
    fn find_by_id(&self, id: &str) -> Option<&Landmark>;
    fn filter(&self, filter: &LandmarkFilter) -> Vec<Landmark>;
    fn nearest(&self, p: WorldPoint, n: usize) -> Vec<NearestLandmark>;
    fn stats(&self) -> &LandmarkStats;
    fn geojson(&self, filter: &LandmarkFilter) -> Value;
    fn predict_ig(&self, irl: LatLon, k: usize) -> Option<Prediction>;
    fn triangulate(&self, rays: &[Ray2D]) -> Option<Triangulation>;
    fn solve_camera_pose(
        &self,
        observations: &[CameraObs],
        width: f64,
        height: f64,
        fov_deg: f64,
    ) -> Option<CameraPose>;
    fn frustum_polygon(
        &self,
        x: f64,
        y: f64,
        yaw_deg: f64,
        fov_deg: f64,
        range_m: f64,
    ) -> Vec<[f64; 2]>;
    fn plan_route(&self, start: WorldPoint, goal: WorldPoint) -> RouteResult;
}

#[derive(Debug, Clone)]
pub struct CoreEngine {
    landmarks: Vec<Landmark>,
    index_by_id: HashMap<String, usize>,
    stats: LandmarkStats,
    irl_model: Option<IrlToIgModel>,
}

impl CoreEngine {
    pub fn from_gta6map_json(raw: &str) -> Result<Self> {
        let rows: Vec<Gta6MapTuple> = serde_json::from_str(raw)?;
        let mut landmarks: Vec<Landmark> = rows
            .iter()
            .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
            .collect();
        landmarks.sort_by_key(|a| id_sort_key(&a.id));
        Ok(Self::from_landmarks(landmarks))
    }

    pub fn from_landmarks(landmarks: Vec<Landmark>) -> Self {
        let mut index_by_id = HashMap::with_capacity(landmarks.len());
        for (i, l) in landmarks.iter().enumerate() {
            index_by_id.insert(l.id.clone(), i);
        }
        let stats = landmark_stats(&landmarks);
        let anchors: Vec<Anchor> = landmarks
            .iter()
            .filter_map(|l| {
                if l.irl_status == IrlStatus::Confirmed {
                    Some(Anchor {
                        irl: l.irl?,
                        ig: l.ig?,
                    })
                } else {
                    None
                }
            })
            .collect();
        let irl_model = build_irl_to_ig_model(&anchors).ok();
        Self {
            landmarks,
            index_by_id,
            stats,
            irl_model,
        }
    }
}

impl LandmarkEngine for CoreEngine {
    fn find_by_id(&self, id: &str) -> Option<&Landmark> {
        self.index_by_id.get(id).map(|&i| &self.landmarks[i])
    }

    fn filter(&self, filter: &LandmarkFilter) -> Vec<Landmark> {
        filter_landmarks(&self.landmarks, filter)
    }

    fn nearest(&self, p: WorldPoint, n: usize) -> Vec<NearestLandmark> {
        nearest_landmarks(&self.landmarks, p, n)
    }

    fn stats(&self) -> &LandmarkStats {
        &self.stats
    }

    fn geojson(&self, filter: &LandmarkFilter) -> Value {
        let filtered = filter_landmarks(&self.landmarks, filter);
        serde_json::to_value(to_geojson(&filtered)).expect("GeoJsonFeatureCollection serializes")
    }

    fn predict_ig(&self, irl: LatLon, k: usize) -> Option<Prediction> {
        let model = self.irl_model.as_ref()?;
        predict_ig(model, irl, k)
    }

    fn triangulate(&self, rays: &[Ray2D]) -> Option<Triangulation> {
        triangulate(rays)
    }

    fn solve_camera_pose(
        &self,
        observations: &[CameraObs],
        width: f64,
        height: f64,
        fov_deg: f64,
    ) -> Option<CameraPose> {
        solve_camera_pose_local(observations, width, height, fov_deg)
    }

    fn frustum_polygon(
        &self,
        x: f64,
        y: f64,
        yaw_deg: f64,
        fov_deg: f64,
        range_m: f64,
    ) -> Vec<[f64; 2]> {
        frustum_polygon_local(x, y, yaw_deg, fov_deg, range_m)
    }

    fn plan_route(&self, start: WorldPoint, goal: WorldPoint) -> RouteResult {
        plan_route_local(&self.landmarks, start, goal)
    }
}

fn project_pinhole(
    obs: &CameraObs,
    params: &[f64; 5],
    width: f64,
    height: f64,
    fl: f64,
) -> Option<[f64; 2]> {
    let [cx, cy, cz, yaw_deg, pitch_deg] = *params;
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg.to_radians();

    let (sy, cy_ang) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();

    // Forward (+y north at yaw=0, +x east at yaw=90)
    let fwd = [sy * cp, cy_ang * cp, sp];
    // Right
    let right = [cy_ang, -sy, 0.0];
    // Up = right x fwd
    let up = [-sy * sp, -cy_ang * sp, cp];

    let dx = obs.world_x - cx;
    let dy = obs.world_y - cy;
    let dz = obs.world_z - cz;

    let cam_z = dx * fwd[0] + dy * fwd[1] + dz * fwd[2];
    if cam_z <= 1.0 {
        return None;
    }
    let cam_x = dx * right[0] + dy * right[1] + dz * right[2];
    let cam_y = dx * up[0] + dy * up[1] + dz * up[2];

    let u = width * 0.5 + fl * (cam_x / cam_z);
    let v = height * 0.5 - fl * (cam_y / cam_z);
    Some([u, v])
}

fn residual_vec(
    observations: &[CameraObs],
    params: &[f64; 5],
    width: f64,
    height: f64,
    fl: f64,
) -> Option<Vec<f64>> {
    let mut res = Vec::with_capacity(observations.len() * 2);
    for obs in observations {
        let [u, v] = project_pinhole(obs, params, width, height, fl)?;
        res.push(u - obs.px);
        res.push(v - obs.py);
    }
    Some(res)
}

fn solve_5x5(mut a: [[f64; 5]; 5], mut b: [f64; 5]) -> Option<[f64; 5]> {
    for i in 0..5 {
        let mut pivot = i;
        let mut max_val = a[i][i].abs();
        for (r, row) in a.iter().enumerate().skip(i + 1) {
            if row[i].abs() > max_val {
                max_val = row[i].abs();
                pivot = r;
            }
        }
        if max_val < 1e-12 {
            return None;
        }
        if pivot != i {
            a.swap(i, pivot);
            b.swap(i, pivot);
        }
        let diag = a[i][i];
        for cell in a[i].iter_mut().skip(i) {
            *cell /= diag;
        }
        b[i] /= diag;
        for r in 0..5 {
            if r == i {
                continue;
            }
            let factor = a[r][i];
            if factor.abs() < 1e-15 {
                continue;
            }
            for c in i..5 {
                a[r][c] -= factor * a[i][c];
            }
            b[r] -= factor * b[i];
        }
    }
    Some(b)
}

fn solve_camera_pose_local(
    observations: &[CameraObs],
    width: f64,
    height: f64,
    fov_deg: f64,
) -> Option<CameraPose> {
    if observations.len() < 3 || width <= 0.0 || height <= 0.0 || !(5.0..175.0).contains(&fov_deg) {
        return None;
    }

    let fl = (width * 0.5) / (fov_deg * 0.5).to_radians().tan();
    let n = observations.len() as f64;
    let mx = observations.iter().map(|o| o.world_x).sum::<f64>() / n;
    let my = observations.iter().map(|o| o.world_y).sum::<f64>() / n;
    let mz = observations.iter().map(|o| o.world_z).sum::<f64>() / n;

    // Try 4 compass quadrants as initial camera positions looking toward centroid
    let mut best_params: Option<[f64; 5]> = None;
    let mut best_cost = f64::INFINITY;

    for yaw_init in [0.0_f64, 90.0, 180.0, 270.0] {
        let r = yaw_init.to_radians();
        let mut params = [
            mx - r.sin() * 600.0,
            my - r.cos() * 600.0,
            mz + 60.0,
            yaw_init,
            -5.0,
        ];
        let mut lambda = 1e-2;
        let Some(mut cur_res) = residual_vec(observations, &params, width, height, fl) else {
            continue;
        };
        let mut cur_cost: f64 = cur_res.iter().map(|v| v * v).sum();

        for _ in 0..45 {
            let mut jtj = [[0.0_f64; 5]; 5];
            let mut jtr = [0.0_f64; 5];
            let mut jac_cols: [Vec<f64>; 5] = Default::default();
            let mut valid = true;

            for (col, item) in jac_cols.iter_mut().enumerate() {
                let eps = if col < 3 { 1e-2 } else { 1e-3 };
                let mut perturbed = params;
                perturbed[col] += eps;
                let Some(p_res) = residual_vec(observations, &perturbed, width, height, fl) else {
                    valid = false;
                    break;
                };
                *item = p_res
                    .iter()
                    .zip(cur_res.iter())
                    .map(|(a, b)| (a - b) / eps)
                    .collect();
            }
            if !valid {
                break;
            }

            for r_idx in 0..cur_res.len() {
                for i in 0..5 {
                    let ji = jac_cols[i][r_idx];
                    jtr[i] -= ji * cur_res[r_idx];
                    for j in 0..5 {
                        jtj[i][j] += ji * jac_cols[j][r_idx];
                    }
                }
            }
            for (i, row) in jtj.iter_mut().enumerate() {
                row[i] += lambda * (row[i].abs().max(1.0));
            }

            let Some(step) = solve_5x5(jtj, jtr) else {
                lambda *= 10.0;
                continue;
            };

            let mut cand = params;
            for i in 0..5 {
                cand[i] += step[i];
            }
            cand[4] = cand[4].clamp(-85.0, 85.0);

            if let Some(next_res) = residual_vec(observations, &cand, width, height, fl) {
                let next_cost: f64 = next_res.iter().map(|v| v * v).sum();
                if next_cost < cur_cost {
                    let improvement = (cur_cost - next_cost).abs();
                    params = cand;
                    cur_res = next_res;
                    cur_cost = next_cost;
                    lambda = (lambda * 0.3).max(1e-6);
                    if improvement < 1e-6 {
                        break;
                    }
                } else {
                    lambda *= 5.0;
                }
            } else {
                lambda *= 10.0;
            }
        }

        if cur_cost < best_cost {
            best_cost = cur_cost;
            best_params = Some(params);
        }
    }

    let p = best_params?;
    let rmse_px = (best_cost / (observations.len() as f64)).sqrt();
    let yaw_deg = ((p[3] % 360.0) + 360.0) % 360.0;
    Some(CameraPose {
        x: p[0],
        y: p[1],
        z: p[2],
        yaw_deg,
        pitch_deg: p[4],
        fov_deg,
        rmse_px,
    })
}

fn frustum_polygon_local(
    x: f64,
    y: f64,
    yaw_deg: f64,
    fov_deg: f64,
    range_m: f64,
) -> Vec<[f64; 2]> {
    let steps = 16usize;
    let half = fov_deg * 0.5;
    let mut pts = Vec::with_capacity(steps + 3);
    pts.push([x, y]);
    for i in 0..=steps {
        let ang = (yaw_deg - half + fov_deg * (i as f64 / steps as f64)).to_radians();
        pts.push([x + ang.sin() * range_m, y + ang.cos() * range_m]);
    }
    pts.push([x, y]);
    pts
}

#[derive(Copy, Clone)]
struct HeapState {
    cost: f64,
    node: usize,
}

impl Eq for HeapState {}

impl PartialEq for HeapState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.node == other.node
    }
}

impl Ord for HeapState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| self.node.cmp(&other.node))
    }
}

impl PartialOrd for HeapState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn plan_route_local(landmarks: &[Landmark], start: WorldPoint, goal: WorldPoint) -> RouteResult {
    let mut nodes: Vec<WorldPoint> = Vec::with_capacity(landmarks.len() + 2);
    nodes.push(start);
    nodes.push(goal);
    for l in landmarks {
        if let Some(p) = l.ig {
            nodes.push(p);
        }
    }

    let n = nodes.len();
    let k = 6usize;
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];

    for i in 0..n {
        let mut dists: Vec<(usize, f64)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (j, (nodes[i].x - nodes[j].x).hypot(nodes[i].y - nodes[j].y)))
            .collect();
        dists.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (j, d) in dists.into_iter().take(k) {
            adj[i].push((j, d));
            adj[j].push((i, d));
        }
    }

    // Ensure direct fallback edge between start (0) and goal (1) if graph is sparse
    let direct_d = (start.x - goal.x).hypot(start.y - goal.y);
    if direct_d <= 1500.0 || n <= 3 {
        adj[0].push((1, direct_d));
    }

    let mut dist = vec![f64::INFINITY; n];
    let mut prev = vec![None; n];
    let mut heap = BinaryHeap::new();
    dist[0] = 0.0;
    heap.push(HeapState { cost: 0.0, node: 0 });

    while let Some(HeapState { cost, node }) = heap.pop() {
        if node == 1 {
            break;
        }
        if cost > dist[node] {
            continue;
        }
        for &(next, w) in &adj[node] {
            let next_cost = cost + w;
            if next_cost < dist[next] {
                dist[next] = next_cost;
                prev[next] = Some(node);
                heap.push(HeapState {
                    cost: next_cost,
                    node: next,
                });
            }
        }
    }

    let mut waypoints = Vec::new();
    if dist[1].is_finite() {
        let mut cur = Some(1usize);
        while let Some(u) = cur {
            waypoints.push([nodes[u].x, nodes[u].y]);
            cur = prev[u];
        }
        waypoints.reverse();
    } else {
        waypoints.push([start.x, start.y]);
        waypoints.push([goal.x, goal.y]);
    }

    let distance_m: f64 = waypoints
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum();
    let drive_min = (distance_m / 1000.0) / 80.0 * 60.0;
    let fly_min = (distance_m / 1000.0) / 220.0 * 60.0;

    RouteResult {
        waypoints,
        distance_m,
        drive_min,
        fly_min,
    }
}
