//! A port of the waypoint graph and distance helpers from JHawk's CaveGen
//! (`buildWayPointGraph`, `closestWayPoint`, `spawnPointDistToStart`, and
//! `pointToSegmentDist` in CaveGen.java).
//!
//! Caveripper has its own waypoint graph for carry paths, but the Judge's
//! scores depend on the exact distances CaveGen computes, so this module
//! mirrors CaveGen's version step by step, including its f32 evaluation
//! order and its quirks.

use crate::{layout::Layout, pikmin_math};

/// `Integer.MAX_VALUE` as CaveGen compares it against floats.
pub(super) const INF: f32 = i32::MAX as f32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl V3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        V3 { x, y, z }
    }
    pub fn length(self) -> f32 {
        pikmin_math::sqrt(self.x * self.x + self.y * self.y + self.z * self.z)
    }
    pub fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, d: f32) -> V3 {
        V3::new(self.x * d, self.y * d, self.z * d)
    }
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
}

/// CaveGen's `spawnPointDist` / `wayPointDist` / `spawnPointWayPointDist`.
pub(super) fn dist(a: V3, b: V3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    let dy = a.y - b.y;
    pikmin_math::sqrt(dx * dx + dy * dy + dz * dz)
}

#[derive(Debug, Clone)]
pub(super) struct Wp {
    pub unit: usize,
    /// Index of this waypoint within its map unit.
    pub idx: usize,
    pub pos: V3,
    pub r: f32,
    pub adj: Vec<usize>,
    inverts: Vec<usize>,
    pub is_start: bool,
    pub back: Option<usize>,
    pub dist_to_start: f32,
    visited: bool,
    /// Set by the Judge: some carryable object's path to the ship passes through here.
    pub carryable_behind: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Graph {
    pub wps: Vec<Wp>,
    pub unit_wps: Vec<Vec<usize>>,
    /// (x, z, width, height) of each map unit in game coordinates.
    unit_boxes: Vec<(f32, f32, f32, f32)>,
    // CaveGen stores these "out" values of pointToSegmentDist in fields, so they
    // keep their old value when the function returns early. That is kept here.
    out_closer: i32,
    out_t: f32,
}

impl Graph {
    pub fn build(layout: &Layout, start: V3) -> Graph {
        let mut wps: Vec<Wp> = Vec::new();
        let mut unit_wps = Vec::new();
        let mut unit_boxes = Vec::new();

        for (ui, unit) in layout.map_units.iter().enumerate() {
            let cx = (unit.x as f32 + unit.unit.width as f32 / 2.0) * 170.0;
            let cz = (unit.z as f32 + unit.unit.height as f32 / 2.0) * 170.0;
            let base = wps.len();
            let mut ids = Vec::new();
            for (i, wp) in unit.unit.waypoints.iter().enumerate() {
                ids.push(base + i);
                wps.push(Wp {
                    unit: ui,
                    idx: wp.index,
                    pos: V3::new(cx + wp.pos[0], wp.pos[1], cz + wp.pos[2]),
                    r: wp.r,
                    adj: wp.links.iter().map(|l| base + l).collect(),
                    inverts: Vec::new(),
                    is_start: false,
                    back: None,
                    dist_to_start: INF,
                    visited: false,
                    carryable_behind: false,
                });
            }
            unit_wps.push(ids);
            unit_boxes.push((
                unit.x as f32 * 170.0,
                unit.z as f32 * 170.0,
                unit.unit.width as f32 * 170.0,
                unit.unit.height as f32 * 170.0,
            ));
        }

        // Links through doors
        for (ui, unit) in layout.map_units.iter().enumerate() {
            for door in unit.doors.iter() {
                let door = door.borrow();
                let a = unit_wps[ui][door.door_unit.waypoint_index];
                let adj = door.adjacent_door.as_ref().unwrap().upgrade().unwrap();
                let adj = adj.borrow();
                let b = unit_wps[adj.parent_idx.unwrap()][adj.door_unit.waypoint_index];
                wps[a].adj.push(b);
            }
        }

        // Inverted links
        for w in 0..wps.len() {
            for k in 0..wps[w].adj.len() {
                let w2 = wps[w].adj[k];
                wps[w2].inverts.push(w);
            }
        }

        let mut graph = Graph {
            wps,
            unit_wps,
            unit_boxes,
            out_closer: 0,
            out_t: 0.0,
        };

        // Starting waypoint: the closest one in the first map unit
        let mut min_dist = INF;
        let mut start_wp = None;
        for &w in graph.unit_wps[0].iter() {
            let d = dist(start, graph.wps[w].pos);
            if d < min_dist {
                min_dist = d;
                start_wp = Some(w);
            }
        }
        let Some(start_wp) = start_wp else { return graph };
        graph.wps[start_wp].is_start = true;
        graph.wps[start_wp].dist_to_start = 0.0;

        // Shortest path to the start from each waypoint
        let mut frontier = vec![start_wp];
        while !frontier.is_empty() {
            let mut best = 0;
            let mut min_dist = INF;
            for (i, &f) in frontier.iter().enumerate() {
                if graph.wps[f].dist_to_start < min_dist {
                    min_dist = graph.wps[f].dist_to_start;
                    best = i;
                }
            }
            let w = frontier.remove(best);
            graph.wps[w].visited = true;

            for k in 0..graph.wps[w].inverts.len() {
                let inv = graph.wps[w].inverts[k];
                if graph.wps[inv].visited {
                    continue;
                }
                if graph.wps[inv].dist_to_start == INF {
                    frontier.push(inv);
                }
                let d = dist(graph.wps[w].pos, graph.wps[inv].pos);
                if d + graph.wps[w].dist_to_start < graph.wps[inv].dist_to_start {
                    graph.wps[inv].back = Some(w);
                    graph.wps[inv].dist_to_start = d + graph.wps[w].dist_to_start;
                }
            }
        }

        graph
    }

    pub fn point_to_segment_dist(&mut self, p: V3, l1: V3, l2: V3, r1: f32, r2: f32) -> f32 {
        let len_l = l2.sub(l1).length();
        if len_l <= 0.0 {
            return 128000.0;
        }
        let diff = l2.sub(l1);
        let len = diff.length();
        let norm = if len <= 0.0 {
            V3::new(0.0, 0.0, 0.0)
        } else {
            diff.scale(1.0 / len)
        };
        let t = norm.dot(p.sub(l1)) / len_l;
        self.out_t = t;
        if t <= 0.0 {
            self.out_closer = 1;
            p.sub(l1).length() - r1
        } else if t >= 1.0 {
            self.out_closer = 2;
            p.sub(l2).length() - r2
        } else {
            self.out_closer = if p.sub(l1).length() - r1 < p.sub(l2).length() - r2 { 1 } else { 2 };
            norm.scale(len_l * t).add(l1).sub(p).length() - (1.0 - t) * r1 - t * r2
        }
    }

    /// CaveGen's `closestWayPoint`.
    pub fn closest_wp(&mut self, p: V3) -> Option<usize> {
        let mut best_dist = INF;
        let mut best_wp = None;
        for ui in 0..self.unit_wps.len() {
            let (x1, y1, w1, h1) = self.unit_boxes[ui];
            if !is_inner_box(x1, y1, w1, h1, p.x - 5.0, p.z - 5.0, 10.0, 10.0) {
                continue;
            }
            for k in 0..self.unit_wps[ui].len() {
                let wp = self.unit_wps[ui][k];
                for a in 0..self.wps[wp].adj.len() {
                    let wp2 = self.wps[wp].adj[a];
                    let (w1, w2) = (&self.wps[wp], &self.wps[wp2]);
                    if w1.idx < w2.idx || !w2.adj.contains(&wp) || w2.unit != ui {
                        let (p1, p2, r1, r2) = (w1.pos, w2.pos, w1.r, w2.r);
                        let d = self.point_to_segment_dist(p, p1, p2, r1, r2);
                        if d < best_dist {
                            best_dist = d;
                            best_wp = Some(if self.out_closer == 1 { wp } else { wp2 });
                        }
                    }
                }
            }
        }
        let best = best_wp?;
        if !self.wps[best].is_start {
            if let Some(back) = self.wps[best].back {
                if self.wps[back].pos == self.wps[best].pos {
                    return Some(back);
                }
            }
        }
        Some(best)
    }

    /// CaveGen's `spawnPointDistToStart`.
    pub fn dist_to_start(&mut self, p: V3) -> f32 {
        let Some(min_wp) = self.closest_wp(p) else { return INF };
        let w = self.wps[min_wp].clone();
        if w.is_start {
            return dist(p, w.pos);
        }
        let Some(back) = w.back else { return INF };
        let b = self.wps[back].clone();
        let d = self.point_to_segment_dist(p, w.pos, b.pos, w.r, b.r);
        let t = self.out_t;
        if t < 0.0 {
            (d + w.r) + w.dist_to_start
        } else if t > 1.0 {
            (d + b.r) + b.dist_to_start
        } else {
            (d + (1.0 - t) * w.r + t * b.r) + b.dist_to_start + (1.0 - t) * (w.dist_to_start - b.dist_to_start)
        }
    }

    /// Marks this waypoint and every waypoint on its path to the ship.
    /// (The propagation half of CaveGen's `computeWaypointCarryableGraph`.)
    pub fn propagate_carryable(&mut self) {
        for unit in 0..self.unit_wps.len() {
            for k in 0..self.unit_wps[unit].len() {
                let w = self.unit_wps[unit][k];
                if !self.wps[w].carryable_behind {
                    continue;
                }
                let mut wp = w;
                loop {
                    self.wps[wp].carryable_behind = true;
                    if self.wps[wp].is_start {
                        break;
                    }
                    match self.wps[wp].back {
                        Some(b) => wp = b,
                        None => break,
                    }
                    if self.wps[wp].carryable_behind {
                        break;
                    }
                }
            }
        }
    }

    pub fn mark_carryable(&mut self, p: V3) {
        if let Some(w) = self.closest_wp(p) {
            self.wps[w].carryable_behind = true;
        }
    }

    pub fn is_carryable(&mut self, p: V3) -> bool {
        self.closest_wp(p).is_some_and(|w| self.wps[w].carryable_behind)
    }
}

fn is_inner_box(x1: f32, y1: f32, w1: f32, h1: f32, x2: f32, y2: f32, w2: f32, h2: f32) -> bool {
    if x1 + w1 <= x2 || x2 + w2 <= x1 {
        return false;
    }
    if y1 + h1 <= y2 || y2 + h2 <= y1 {
        return false;
    }
    true
}
