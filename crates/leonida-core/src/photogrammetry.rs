//! Photogrammetry camera pose calibration and FOV frustum polygon generation.

use serde::{Deserialize, Serialize};

/// A 3D-to-2D correspondence between a world landmark and a screenshot pixel coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraObs {
    pub world_x: f64,
    pub world_y: f64,
    pub world_z: f64,
    pub px: f64,
    pub py: f64,
}

/// Estimated camera pose in Leonida world coordinates (`+x` east, `+y` north, `+z` up).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

fn camera_axes(yaw_deg: f64, pitch_deg: f64) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let y = yaw_deg.to_radians();
    let p = pitch_deg.to_radians();
    let (sy, cy) = y.sin_cos();
    let (sp, cp) = p.sin_cos();

    let forward = [sy * cp, cy * cp, sp];
    let right = [cy, -sy, 0.0];
    let up = [-sy * sp, -cy * sp, cp];
    (right, up, forward)
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Project a world point into pixel coordinates for a candidate pose.
/// Returns `None` if the point lies behind or on the camera plane.
pub fn project_world_to_pixel(
    pose: &CameraPose,
    world: [f64; 3],
    width: f64,
    height: f64,
) -> Option<(f64, f64)> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let half_fov = (pose.fov_deg.clamp(5.0, 175.0) * 0.5).to_radians();
    let focal = (width * 0.5) / half_fov.tan();
    let (right, up, forward) = camera_axes(pose.yaw_deg, pose.pitch_deg);

    let d = [world[0] - pose.x, world[1] - pose.y, world[2] - pose.z];
    let zc = dot3(d, forward);
    if zc <= 1e-3 {
        return None;
    }
    let xc = dot3(d, right);
    let yc = dot3(d, up);

    let px = width * 0.5 + focal * (xc / zc);
    let py = height * 0.5 - focal * (yc / zc);
    Some((px, py))
}

fn eval_residuals(
    params: &[f64; 6],
    obs: &[CameraObs],
    width: f64,
    height: f64,
) -> Option<(Vec<f64>, f64)> {
    let pose = CameraPose {
        x: params[0],
        y: params[1],
        z: params[2],
        yaw_deg: params[3],
        pitch_deg: params[4].clamp(-89.0, 89.0),
        fov_deg: params[5].clamp(10.0, 160.0),
        rmse_px: 0.0,
    };

    let mut res = Vec::with_capacity(obs.len() * 2);
    let mut sse = 0.0;
    for o in obs {
        let (px, py) =
            project_world_to_pixel(&pose, [o.world_x, o.world_y, o.world_z], width, height)?;
        let rx = px - o.px;
        let ry = py - o.py;
        sse += rx * rx + ry * ry;
        res.push(rx);
        res.push(ry);
    }
    Some((res, sse))
}

fn solve_linear<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let mut pivot_row = col;
        let mut pivot_val = a[col][col].abs();
        for (r, row) in a.iter().enumerate().skip(col + 1) {
            let v = row[col].abs();
            if v > pivot_val {
                pivot_val = v;
                pivot_row = r;
            }
        }
        if pivot_val < 1e-12 || !pivot_val.is_finite() {
            return None;
        }
        if pivot_row != col {
            a.swap(col, pivot_row);
            b.swap(col, pivot_row);
        }
        let diag = a[col][col];
        for r in (col + 1)..N {
            let factor = a[r][col] / diag;
            for c in col..N {
                a[r][c] -= factor * a[col][c];
            }
            b[r] -= factor * b[col];
        }
    }

    let mut x = [0.0; N];
    for i in (0..N).rev() {
        let mut sum = b[i];
        for j in (i + 1)..N {
            sum -= a[i][j] * x[j];
        }
        x[i] = sum / a[i][i];
        if !x[i].is_finite() {
            return None;
        }
    }
    Some(x)
}

fn lm_refine(
    mut params: [f64; 6],
    obs: &[CameraObs],
    width: f64,
    height: f64,
    optimize_fov: bool,
) -> Option<([f64; 6], f64)> {
    let (_, mut sse) = eval_residuals(&params, obs, width, height)?;
    let mut lambda = 1e-3_f64;
    let steps = [0.5_f64, 0.5, 0.5, 0.05, 0.05, 0.05];

    for _ in 0..80 {
        let (res, _) = eval_residuals(&params, obs, width, height)?;
        let m = res.len();
        let dof = if optimize_fov { 6 } else { 5 };

        let mut jtj = [[0.0_f64; 6]; 6];
        let mut jtr = [0.0_f64; 6];
        let mut cols = vec![[0.0_f64; 6]; m];

        for p_idx in 0..dof {
            let h = steps[p_idx];
            let mut p_hi = params;
            let mut p_lo = params;
            p_hi[p_idx] += h;
            p_lo[p_idx] -= h;

            let (r_hi, _) = eval_residuals(&p_hi, obs, width, height)?;
            let (r_lo, _) = eval_residuals(&p_lo, obs, width, height)?;
            for i in 0..m {
                cols[i][p_idx] = (r_hi[i] - r_lo[i]) / (2.0 * h);
            }
        }

        for i in 0..m {
            for r in 0..dof {
                jtr[r] += cols[i][r] * res[i];
                for c in 0..dof {
                    jtj[r][c] += cols[i][r] * cols[i][c];
                }
            }
        }
        if !optimize_fov {
            jtj[5][5] = 1.0;
        }
        for (idx, row) in jtj.iter_mut().enumerate().take(dof) {
            row[idx] += lambda * (row[idx].abs().max(1.0));
        }

        let Some(delta) = solve_linear(jtj, jtr) else {
            lambda *= 10.0;
            if lambda > 1e9 {
                break;
            }
            continue;
        };

        let mut cand = params;
        for idx in 0..dof {
            cand[idx] -= delta[idx];
        }
        cand[4] = cand[4].clamp(-89.0, 89.0);
        cand[5] = cand[5].clamp(10.0, 160.0);

        if let Some((_, cand_sse)) = eval_residuals(&cand, obs, width, height) {
            if cand_sse < sse {
                let rel_improv = (sse - cand_sse) / sse.max(1.0);
                params = cand;
                sse = cand_sse;
                lambda = (lambda * 0.3).max(1e-7);
                if rel_improv < 1e-10 {
                    break;
                }
                continue;
            }
        }
        lambda *= 5.0;
        if lambda > 1e9 {
            break;
        }
    }

    Some((params, (sse / obs.len() as f64).sqrt()))
}

/// Estimate camera pose `(x, y, z, yaw_deg, pitch_deg, fov_deg, rmse_px)` from 3D-2D correspondences.
///
/// Requires at least 3 finite observations and positive `(width, height)`.
pub fn solve_camera_pose(
    obs: &[CameraObs],
    width: f64,
    height: f64,
    initial_fov_deg: Option<f64>,
) -> Option<CameraPose> {
    if obs.len() < 3 || !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    if obs.iter().any(|o| {
        !o.world_x.is_finite()
            || !o.world_y.is_finite()
            || !o.world_z.is_finite()
            || !o.px.is_finite()
            || !o.py.is_finite()
    }) {
        return None;
    }

    let fov0 = initial_fov_deg.unwrap_or(60.0).clamp(10.0, 160.0);
    let n = obs.len() as f64;
    let mx = obs.iter().map(|o| o.world_x).sum::<f64>() / n;
    let my = obs.iter().map(|o| o.world_y).sum::<f64>() / n;
    let mz = obs.iter().map(|o| o.world_z).sum::<f64>() / n;

    let spread = obs
        .iter()
        .map(|o| (o.world_x - mx).hypot(o.world_y - my))
        .fold(100.0_f64, f64::max);

    let mut best: Option<([f64; 6], f64)> = None;
    for step in 0..12 {
        let az = (step as f64) * 30.0_f64;
        let rad = az.to_radians();
        let dist = spread * 2.0;
        let cx = mx - dist * rad.sin();
        let cy = my - dist * rad.cos();
        let cz = mz + dist * 0.25;
        let pitch = -(cz - mz).atan2(dist).to_degrees();
        let init = [cx, cy, cz, az, pitch, fov0];

        if let Some((_, sse)) = eval_residuals(&init, obs, width, height) {
            if best.is_none_or(|(_, b_sse)| sse < b_sse) {
                best = Some((init, sse));
            }
        }
    }

    let (init_params, _) = best?;
    let (refined, _) = lm_refine(init_params, obs, width, height, false)?;
    let (final_params, rmse_px) = if obs.len() >= 4 {
        lm_refine(refined, obs, width, height, true).unwrap_or((refined, f64::INFINITY))
    } else {
        let (_, sse) = eval_residuals(&refined, obs, width, height)?;
        (refined, (sse / n).sqrt())
    };

    let yaw = final_params[3].rem_euclid(360.0);
    Some(CameraPose {
        x: final_params[0],
        y: final_params[1],
        z: final_params[2],
        yaw_deg: if yaw >= 360.0 { 0.0 } else { yaw },
        pitch_deg: final_params[4].clamp(-89.0, 89.0),
        fov_deg: final_params[5].clamp(10.0, 160.0),
        rmse_px,
    })
}

/// Generate a 2D map-space FOV sector polygon `[x, y]` for rendering a camera frustum.
pub fn frustum_polygon(x: f64, y: f64, yaw_deg: f64, fov_deg: f64, range_m: f64) -> Vec<[f64; 2]> {
    if !x.is_finite()
        || !y.is_finite()
        || !yaw_deg.is_finite()
        || !fov_deg.is_finite()
        || !range_m.is_finite()
        || range_m <= 0.0
    {
        return Vec::new();
    }

    let half_fov = fov_deg.clamp(1.0, 179.0) * 0.5;
    let steps = 16usize;
    let mut poly = Vec::with_capacity(steps + 3);
    poly.push([x, y]);
    for i in 0..=steps {
        let frac = (i as f64) / (steps as f64);
        let angle = (yaw_deg - half_fov + frac * (2.0 * half_fov)).to_radians();
        poly.push([x + range_m * angle.sin(), y + range_m * angle.cos()]);
    }
    poly.push([x, y]);
    poly
}
