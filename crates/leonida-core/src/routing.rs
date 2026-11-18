//! KNN graph construction and Dijkstra shortest-path routing across Leonida landmarks.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};

use crate::landmarks::Landmark;

/// Driving speed assumption in world metres per minute (72 km/h).
const DRIVE_METRES_PER_MIN: f64 = 1200.0;
/// Flying speed assumption in world metres per minute (180 km/h).
const FLY_METRES_PER_MIN: f64 = 3000.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteResult {
    pub waypoints: Vec<[f64; 2]>,
    pub distance_m: f64,
    pub drive_min: f64,
    pub fly_min: f64,
}

#[derive(Debug, Clone, Copy)]
struct HeapState {
    cost: f64,
    node: usize,
}

impl PartialEq for HeapState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.node == other.node
    }
}

impl Eq for HeapState {}

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

fn dist2d(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn make_direct_route(start: [f64; 2], end: [f64; 2]) -> RouteResult {
    let distance_m = dist2d(start, end);
    RouteResult {
        waypoints: vec![start, end],
        distance_m,
        drive_min: distance_m / DRIVE_METRES_PER_MIN,
        fly_min: distance_m / FLY_METRES_PER_MIN,
    }
}

/// Plan a shortest path between `start` and `end` over a `k`-nearest-neighbour graph of `nodes`.
///
/// Falls back to a direct `[start, end]` segment when `nodes` is empty or the graph is disconnected.
pub fn plan_route(
    nodes: &[[f64; 2]],
    start: [f64; 2],
    end: [f64; 2],
    k: usize,
) -> Option<RouteResult> {
    if !start[0].is_finite() || !start[1].is_finite() || !end[0].is_finite() || !end[1].is_finite()
    {
        return None;
    }

    let valid_nodes: Vec<[f64; 2]> = nodes
        .iter()
        .copied()
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .collect();

    if valid_nodes.is_empty() {
        return Some(make_direct_route(start, end));
    }

    let n = valid_nodes.len();
    let start_idx = n;
    let end_idx = n + 1;
    let total = n + 2;
    let knn = k.max(1).min(n);

    let mut pts = Vec::with_capacity(total);
    pts.extend_from_slice(&valid_nodes);
    pts.push(start);
    pts.push(end);

    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); total];

    // Connect each landmark node to its k nearest landmark neighbours.
    for i in 0..n {
        let mut neigh: Vec<(usize, f64)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (j, dist2d(pts[i], pts[j])))
            .collect();
        neigh.sort_by(|a, b| a.1.total_cmp(&b.1));
        neigh.truncate(knn);
        for (j, d) in neigh {
            adj[i].push((j, d));
            adj[j].push((i, d));
        }
    }

    // Connect start and end to their k nearest landmark nodes.
    for &endpoint in &[start_idx, end_idx] {
        let mut neigh: Vec<(usize, f64)> =
            (0..n).map(|j| (j, dist2d(pts[endpoint], pts[j]))).collect();
        neigh.sort_by(|a, b| a.1.total_cmp(&b.1));
        neigh.truncate(knn);
        for (j, d) in neigh {
            adj[endpoint].push((j, d));
            adj[j].push((endpoint, d));
        }
    }

    // Dijkstra from start_idx to end_idx.
    let mut dist = vec![f64::INFINITY; total];
    let mut prev: Vec<Option<usize>> = vec![None; total];
    let mut heap = BinaryHeap::new();

    dist[start_idx] = 0.0;
    heap.push(HeapState {
        cost: 0.0,
        node: start_idx,
    });

    while let Some(HeapState { cost, node }) = heap.pop() {
        if node == end_idx {
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

    let total_dist = dist[end_idx];
    if !total_dist.is_finite() {
        return Some(make_direct_route(start, end));
    }

    let mut path_indices = Vec::new();
    let mut cur = Some(end_idx);
    while let Some(idx) = cur {
        path_indices.push(idx);
        if idx == start_idx {
            break;
        }
        cur = prev[idx];
    }
    path_indices.reverse();

    if path_indices.first().copied() != Some(start_idx) {
        return Some(make_direct_route(start, end));
    }

    let waypoints: Vec<[f64; 2]> = path_indices.into_iter().map(|i| pts[i]).collect();
    Some(RouteResult {
        waypoints,
        distance_m: total_dist,
        drive_min: total_dist / DRIVE_METRES_PER_MIN,
        fly_min: total_dist / FLY_METRES_PER_MIN,
    })
}

/// Convenience wrapper to plan a route using positioned [`Landmark`] entries as graph nodes.
pub fn plan_route_from_landmarks(
    landmarks: &[Landmark],
    start: [f64; 2],
    end: [f64; 2],
    k: usize,
) -> Option<RouteResult> {
    let nodes: Vec<[f64; 2]> = landmarks
        .iter()
        .filter_map(|l| l.ig.map(|p| [p.x, p.y]))
        .collect();
    plan_route(&nodes, start, end, k)
}
