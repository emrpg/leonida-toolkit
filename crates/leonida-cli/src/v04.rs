use leonida_core::WorldPoint;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct CameraObs {
    pub world_x: f64,
    pub world_y: f64,
    pub world_z: f64,
    pub px: f64,
    pub py: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraPose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub fov_deg: f64,
    pub rmse_px: f64,
}

fn project_obs(
    params: &[f64; 5],
    fov_deg: f64,
    width: f64,
    height: f64,
    obs: &CameraObs,
) -> Option<(f64, f64)> {
    let [cx, cy, cz, yaw_deg, pitch_deg] = *params;
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg.to_radians();

    let fx = yaw.sin() * pitch.cos();
    let fy = yaw.cos() * pitch.cos();
    let fz = pitch.sin();

    let rx = yaw.cos();
    let ry = -yaw.sin();

    let ux = -yaw.sin() * pitch.sin();
    let uy = -yaw.cos() * pitch.sin();
    let uz = pitch.cos();

    let dx = obs.world_x - cx;
    let dy = obs.world_y - cy;
    let dz = obs.world_z - cz;

    let cam_z = dx * fx + dy * fy + dz * fz;
    if cam_z <= 1e-3 {
        return None;
    }
    let cam_x = dx * rx + dy * ry;
    let cam_y = dx * ux + dy * uy + dz * uz;

    let half_fov = (fov_deg.clamp(10.0, 150.0) * 0.5).to_radians();
    let f_px = (width * 0.5) / half_fov.tan();
    let u = width * 0.5 + f_px * (cam_x / cam_z);
    let v = height * 0.5 - f_px * (cam_y / cam_z);
    Some((u, v))
}

fn eval_sse(params: &[f64; 5], fov_deg: f64, width: f64, height: f64, obs: &[CameraObs]) -> f64 {
    let mut sse = 0.0;
    for o in obs {
        match project_obs(params, fov_deg, width, height, o) {
            Some((u, v)) => {
                let du = u - o.px;
                let dv = v - o.py;
                sse += du * du + dv * dv;
            }
            None => sse += 1e9,
        }
    }
    sse
}

pub fn solve_camera_pose(
    obs: &[CameraObs],
    width: f64,
    height: f64,
    fov_deg: Option<f64>,
) -> Option<CameraPose> {
    if obs.len() < 3 || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let fov = fov_deg.unwrap_or(60.0).clamp(10.0, 150.0);

    let n = obs.len() as f64;
    let mx = obs.iter().map(|o| o.world_x).sum::<f64>() / n;
    let my = obs.iter().map(|o| o.world_y).sum::<f64>() / n;
    let mz = obs.iter().map(|o| o.world_z).sum::<f64>() / n;

    let mut best = [mx, my - 500.0, mz + 50.0, 0.0, -5.0];
    let mut best_sse = eval_sse(&best, fov, width, height, obs);

    for k in 0..8 {
        let yaw = (k as f64) * 45.0;
        let rad = yaw.to_radians();
        let cand = [
            mx - 500.0 * rad.sin(),
            my - 500.0 * rad.cos(),
            mz + 50.0,
            yaw,
            -5.0,
        ];
        let sse = eval_sse(&cand, fov, width, height, obs);
        if sse < best_sse {
            best_sse = sse;
            best = cand;
        }
    }

    let mut steps = [200.0, 200.0, 100.0, 15.0, 10.0];
    for _ in 0..64 {
        let mut improved = false;
        for i in 0..5 {
            for dir in [-1.0, 1.0] {
                let mut cand = best;
                cand[i] += dir * steps[i];
                if i == 4 {
                    cand[4] = cand[4].clamp(-85.0, 85.0);
                }
                let sse = eval_sse(&cand, fov, width, height, obs);
                if sse < best_sse {
                    best_sse = sse;
                    best = cand;
                    improved = true;
                }
            }
        }
        if !improved {
            for s in &mut steps {
                *s *= 0.5;
            }
        }
    }

    let yaw_norm = ((best[3] % 360.0) + 360.0) % 360.0;
    let rmse_px = (best_sse / n).sqrt();
    Some(CameraPose {
        x: best[0],
        y: best[1],
        z: best[2],
        yaw_deg: yaw_norm,
        pitch_deg: best[4],
        fov_deg: fov,
        rmse_px,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    pub waypoints: Vec<[f64; 2]>,
    pub distance_m: f64,
    pub drive_min: f64,
    pub fly_min: f64,
}

#[derive(Copy, Clone, PartialEq)]
struct State {
    cost: f64,
    idx: usize,
}

impl Eq for State {}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.total_cmp(&self.cost)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn plan_route(
    nodes: &[WorldPoint],
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    k: usize,
) -> RouteResult {
    let mut pts = Vec::with_capacity(nodes.len() + 2);
    pts.push(WorldPoint { x: x1, y: y1 });
    pts.extend_from_slice(nodes);
    pts.push(WorldPoint { x: x2, y: y2 });

    let n = pts.len();
    let goal = n - 1;
    let k = k.max(2).min(n.saturating_sub(1));

    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut seen: HashSet<(usize, usize)> = HashSet::new();

    for i in 0..n {
        let mut dists: Vec<(usize, f64)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (j, (pts[i].x - pts[j].x).hypot(pts[i].y - pts[j].y)))
            .collect();
        dists.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (j, d) in dists.into_iter().take(k) {
            let edge = if i < j { (i, j) } else { (j, i) };
            if seen.insert(edge) {
                adj[i].push((j, d));
                adj[j].push((i, d));
            }
        }
    }

    let mut dist = vec![f64::INFINITY; n];
    let mut prev = vec![None; n];
    let mut heap = BinaryHeap::new();
    dist[0] = 0.0;
    heap.push(State { cost: 0.0, idx: 0 });

    while let Some(State { cost, idx }) = heap.pop() {
        if idx == goal {
            break;
        }
        if cost > dist[idx] {
            continue;
        }
        for &(next, w) in &adj[idx] {
            let next_cost = cost + w;
            if next_cost < dist[next] {
                dist[next] = next_cost;
                prev[next] = Some(idx);
                heap.push(State {
                    cost: next_cost,
                    idx: next,
                });
            }
        }
    }

    let (waypoints, distance_m) = if dist[goal].is_finite() {
        let mut path = Vec::new();
        let mut cur = Some(goal);
        while let Some(idx) = cur {
            path.push([pts[idx].x, pts[idx].y]);
            cur = prev[idx];
        }
        path.reverse();
        (path, dist[goal])
    } else {
        let d = (x2 - x1).hypot(y2 - y1);
        (vec![[x1, y1], [x2, y2]], d)
    };

    let drive_min = distance_m / (80_000.0 / 60.0);
    let fly_min = distance_m / (220_000.0 / 60.0);
    RouteResult {
        waypoints,
        distance_m,
        drive_min,
        fly_min,
    }
}
