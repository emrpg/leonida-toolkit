use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HeatPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeatmapCell {
    pub x: f64,
    pub y: f64,
    pub count: usize,
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionSample {
    pub x: f64,
    pub y: f64,
    pub bearing: f64,
    pub progress: f64,
}

pub fn compute_heatmap_cells(points: &[HeatPoint], cell_size: f64) -> Vec<HeatmapCell> {
    if points.is_empty() {
        return Vec::new();
    }
    let size = cell_size.max(10.0);
    let mut grid: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    let mut max_count = 0usize;

    for p in points {
        if !p.x.is_finite() || !p.y.is_finite() {
            continue;
        }
        let ix = (p.x / size).floor() as i64;
        let iy = (p.y / size).floor() as i64;
        let c = grid.entry((ix, iy)).or_insert(0);
        *c += 1;
        if *c > max_count {
            max_count = *c;
        }
    }

    if max_count == 0 {
        return Vec::new();
    }

    let denom = max_count as f64;
    grid.into_iter()
        .map(|((ix, iy), count)| HeatmapCell {
            x: (ix as f64 + 0.5) * size,
            y: (iy as f64 + 0.5) * size,
            count,
            weight: (count as f64 / denom).clamp(0.0, 1.0),
        })
        .collect()
}

pub fn interpolate_motion_path(points: &[HeatPoint], t: f64) -> Option<MotionSample> {
    if points.is_empty() {
        return None;
    }
    let progress = t.clamp(0.0, 1.0);
    if points.len() == 1 {
        return Some(MotionSample {
            x: points[0].x,
            y: points[0].y,
            bearing: 0.0,
            progress,
        });
    }

    let mut seg_lens = Vec::with_capacity(points.len() - 1);
    let mut total_len = 0.0;
    for w in points.windows(2) {
        let len = (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
        seg_lens.push(len);
        total_len += len;
    }

    if total_len < 1e-9 {
        let last = points[points.len() - 1];
        return Some(MotionSample {
            x: last.x,
            y: last.y,
            bearing: 0.0,
            progress,
        });
    }

    let target = progress * total_len;
    let mut acc = 0.0;
    for (i, &seg_len) in seg_lens.iter().enumerate() {
        let a = points[i];
        let b = points[i + 1];
        if acc + seg_len >= target || i + 1 == seg_lens.len() {
            let local_t = if seg_len > 1e-9 {
                ((target - acc) / seg_len).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let bearing = dx.atan2(dy).to_degrees().rem_euclid(360.0);
            return Some(MotionSample {
                x: a.x + dx * local_t,
                y: a.y + dy * local_t,
                bearing,
                progress,
            });
        }
        acc += seg_len;
    }

    None
}
