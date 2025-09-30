use std::{iter, ops::{Add, Sub}};

use crate::point::Point;
use float_ord::FloatOrd;
use itertools::Itertools;
use petgraph::{prelude::NodeIndex, visit::EdgeRef, Direction, Graph};

use super::{Layout, SpawnObject};

#[derive(Debug, Clone)]
pub struct WaypointGraph {
    graph: Graph<WaypointGraphNode, f32>,
}

impl WaypointGraph {
    pub(super) fn build(layout: &Layout) -> Self {
        let mut graph = Graph::<WaypointGraphNode, f32>::new();

        // Connect waypoints within map units
        let nodes_per_unit = layout
            .map_units
            .iter()
            .map(|map_unit| {
                let nodes = map_unit
                    .unit
                    .waypoints
                    .iter()
                    .map(|wp| {
                        let node = graph.add_node(WaypointGraphNode {
                            dist_to_start: f32::MAX,
                            idx: NodeIndex::new(0),
                            visited: false,
                            // Transform the center-of-room waypoint coordinates into global coordinates
                            pos: wp.pos
                                + Point([
                                    (map_unit.x as f32 + map_unit.unit.width as f32 / 2.0) * 170.0,
                                    0.0,
                                    (map_unit.z as f32 + map_unit.unit.height as f32 / 2.0) * 170.0,
                                ]),
                            r: wp.r,
                        });
                        graph[node].idx = node;
                        node
                    })
                    .collect_vec();

                for (wp, node) in map_unit.unit.waypoints.iter().zip(nodes.iter()) {
                    for link in wp.links.iter() {
                        graph.add_edge(
                            *node,
                            nodes[*link],
                            graph[*node].p2_dist(&graph[nodes[*link]]),
                        );
                    }
                }

                nodes
            })
            .collect_vec();

        // Connect doors between map units
        for map_unit in layout.map_units.iter() {
            for door in map_unit.doors.iter() {
                let node = nodes_per_unit[door.borrow().parent_idx.unwrap()]
                    [door.borrow().door_unit.waypoint_index];
                let adj_door = door
                    .borrow()
                    .adjacent_door
                    .as_ref()
                    .unwrap()
                    .upgrade()
                    .unwrap();
                let adj_unit_idx = adj_door.borrow().parent_idx.unwrap();
                let adj_node =
                    nodes_per_unit[adj_unit_idx][adj_door.borrow().door_unit.waypoint_index];
                graph.add_edge(node, adj_node, f32::MAX);
            }
        }

        // Find start point
        let start_location = layout
            .get_spawn_objects()
            .find(|so| matches!(so.0, SpawnObject::Ship))
            .unwrap()
            .1;
        let start_wp = graph
            .node_indices()
            .min_by_key(|wp| {
                let wp = &graph[*wp];
                FloatOrd(wp.pos.p2_dist(&start_location))
            })
            .unwrap();
        graph[start_wp].dist_to_start = 0.0;

        // Expand the frontier, marking distances and backlinks along the way
        let mut frontier = vec![start_wp];
        while !frontier.is_empty() {
            frontier.sort_by_key(|wp| {
                let wp = &graph[*wp];
                FloatOrd(wp.pos.p2_dist(&start_location) * -1.0) // Sort backwards
            });
            let closest = frontier.pop().unwrap();
            graph[closest].visited = true;

            let mut walker = graph
                .neighbors_directed(closest, Direction::Incoming)
                .detach();
            while let Some((_, invert)) = walker.next(&graph) {
                if graph[invert].visited {
                    continue;
                }
                if graph[invert].dist_to_start == f32::MAX {
                    frontier.push(invert);
                }
                let dist = graph[closest].p2_dist(&graph[invert]) + graph[closest].dist_to_start;
                if dist < graph[invert].dist_to_start {
                    graph.add_edge(invert, closest, graph[invert].p2_dist(&graph[closest]));
                    graph[invert].dist_to_start = dist;
                }
            }
        }

        // Remove outgoing nodes from the start waypoint to prevent a cyclic path
        let edges_to_remove = graph
            .edges_directed(start_wp, Direction::Outgoing)
            .map(|e| e.id())
            .collect_vec();
        edges_to_remove.into_iter().for_each(|e| {
            graph.remove_edge(e);
        });

        Self { graph }
    }

    pub fn iter(&self) -> impl Iterator<Item = &WaypointGraphNode> {
        self.graph.node_weights()
    }

    /// The waypoint a carrier should take from this waypoint to get back to the ship
    pub fn backlink(&self, wp: &WaypointGraphNode) -> Option<&WaypointGraphNode> {
        self.graph
            .neighbors_directed(wp.idx, Direction::Outgoing)
            .next()
            .map(|idx| &self.graph[idx])
    }

    /// The full chain of waypoints that should be taken from the provided point to get back to the ship
    pub fn carry_path_wps_nodes(&self, pos: Point<3, f32>) -> impl Iterator<Item = &WaypointGraphNode> + '_ {

        // JHawk pathfinder - compares every edge between waypoints, and finds the closest one to the starting point
        // Then takes the closest of the two waypoints along that edge
        let mut best_dist = 128000.0;
        let mut best_wp: &WaypointGraphNode = &self.graph[NodeIndex::new(0)];

        // For every waypoint in the original graph
        for wp1 in self.iter() {
            // Get all the neighbors of wp1 (weird rust way of doing this - make a map with the node index of each neighbor to it's appropiate node in the graph)
            // We are drawing lines from wp1 to the neighbors to see which is the closest to the starting point
            for wp2 in self.graph.neighbors(wp1.idx).map(|wp_next_idx| &self.graph[wp_next_idx]) {
                // Note to self: don't assign value here cause rust compiler will complain with unused varibale warning then maya will complain of warnings
                let d;  
                let len_j = (wp2.pos - wp1.pos).length();
                // If too short a dist, don't bother with this check
                if len_j <= 0.0
                {
                    continue;
                }
                let norm_j = (wp2.pos - wp1.pos).normalized();
                let t_j = norm_j.dot(pos - wp1.pos) / len_j;
                // let mut point_to_segment_dist_out_t = t_j; <--- this is unused in our code but jhawk calculates this, might as well keep it?
                let point_to_segment_dist_out_closer_vec;
                // With the line between wp1 and wp2, determine which of the two waypoints we are closer to
                if t_j <= 0.0 { // way off the line, close to wp1
                    point_to_segment_dist_out_closer_vec = 1;
                    d = (pos - wp1.pos).length() - wp1.r;
                } else if t_j >= 1.0 { // way off the line, close to wp2
                    point_to_segment_dist_out_closer_vec = 2;
                    d = (pos - wp2.pos).length() - wp2.r;
                } else { // somewhere in the line between wp1 and wp2, do more math to really see who's closer
                    point_to_segment_dist_out_closer_vec = if ((pos - wp1.pos).length() - wp1.r) < ( (pos - wp2.pos).length() - wp2.r) {
                        1
                    } else {
                        2
                    };
                    d = (((norm_j * (len_j * t_j)) + wp1.pos)- pos).length() - (1.0-t_j)*wp1.r - t_j*2.0;
                }
                // Check if our current distance is the closest so far (this is the closest waypoint line to our starting position!!)
                if d < best_dist {
                    best_dist = d;
                    // Decide which of the two waypoints on the closest line is closest to us!
                    best_wp = if point_to_segment_dist_out_closer_vec == 1 {
                        wp1
                    } else {
                        wp2
                    };
                }
            }
        }

        // This part is the same - just make a list of all the nodes starting from our closest, heading to the ship
        let mut ret: Vec<&WaypointGraphNode> = vec![best_wp];
        while let Some(backlink) = self.backlink(ret.last().unwrap()) {
            ret.push(backlink);
            if ret[ret.len() - 1].pos == ret[ret.len() - 2].pos {
                ret.remove(ret.len() - 2);
            }
        }
        ret.into_iter()
    }

    /// Same as above, but returns just the waypoint coordinates instead of the entire waypoint node
    pub fn carry_path_wps_pos(&self, pos: Point<3, f32>) -> impl Iterator<Item = Point<3, f32>> + '_ {
        iter::once(pos).chain(self.carry_path_wps_nodes(pos).map(|wp| wp.pos))
    }

}

#[derive(Debug, Clone)]
pub struct WaypointGraphNode {
    pub dist_to_start: f32,
    idx: NodeIndex,
    visited: bool,
    pub pos: Point<3, f32>,
    pub r: f32,
}

impl WaypointGraphNode {
    pub fn p2_dist(&self, other: &Self) -> f32 {
        self.pos.p2_dist(&other.pos)
    }
}

/** THE FUNCTION™ - makes a smooth line representing the path pikmin take from a given treasure to the ship
 *  Takes in the starting position, a "speed" value, max number of iterations for searching, and the collection of waypoints to path through
 */
pub fn get_path_to_goal (
    start: Point::<3, f32>,
    speed: f32,
    max_num_iter: i32,
    path: Vec<&WaypointGraphNode>,
) ->  Vec<Point::<3, f32>> {
    // This is the final path of points! Lets start with our starting point (duh)
    let mut ret_path: Vec<Point::<3, f32>> = Vec::new();
    ret_path.push(start);

    // Safety check; don't make a path if there's no path to make
    if path.len() == 0 {
        return ret_path;
    }

    // Setup some vars for the spline stuff - copied from jhawk's logic
    let mut cur_path_node: i32 = -1;
    let mut goal_mode: bool = false;
    let mut cur_pos: Point<3, f32> = start;
    let mut cur_vel = Point::<3, f32>([0.0, 0.0, 0.0]);

    let goal_pos: Point<3, f32> = path[path.len() - 1].pos;
    let mut t0: Point<3, f32>;
    let mut t1: Point<3, f32>;
    let mut t2: Point<3, f32>;
    let mut t3: Point<3, f32>;

    // Initial Logic
    {
        // Jhawk calls this CRMakeRefs - I'm guessing this is an edge case of path size of 1 waypoint?
        if path.len() <= 1 {
            t0 = if cur_path_node - 1 <= -1 {
                cur_pos
            } else {
                if cur_path_node - 1 >= path.len().try_into().unwrap(){
                    goal_pos
                } else {
                    path[(cur_path_node - 1) as usize].pos
                }
            };

            t1 = if cur_path_node <= -1 {
                cur_pos
            } else {
                if cur_path_node >= path.len().try_into().unwrap(){
                    goal_pos
                } else {
                    path[cur_path_node as usize].pos
                }
            };

            t2 = if cur_path_node + 1 <= -1 {
                cur_pos
            } else {
                if cur_path_node + 1 >= path.len().try_into().unwrap(){
                    goal_pos
                } else {
                    path[(cur_path_node + 1) as usize].pos
                }
            };

            t3 = if cur_path_node + 2 <= -1 {
                cur_pos
            } else {
                if cur_path_node + 2 >= path.len().try_into().unwrap(){
                    goal_pos
                } else {
                    path[(cur_path_node + 2) as usize].pos
                }
            };
        } else {
            // More jhawk variables
            let cur_vec: Point<3, f32> = path[0].pos;
            let next_vec: Point<3, f32> = path[1].pos;
            let d: Point<3, f32> = (next_vec - cur_vec).normalized();
            let len_next_cur: f32 = cur_vec.dist(&next_vec);

            let t: f32 = cur_pos.sub(cur_vec).dot(d) / len_next_cur;

            let cur_radius = path[0].r;
            let next_radius = path[1].r;
            let adj_radius = ((1 as f32)-t) * cur_radius + t * next_radius; // this cast is so dumb maya why is rust like this?!

            let n_full = (d * (t * len_next_cur)).add(cur_vec).sub(cur_pos);
            let mut len_n = n_full.length();
            // Don't let n be zero for math reasons, so just set it to a tiny number
            if len_n == 0.0 {
                len_n = 0.0001;
            }

            // "tube collides" - jhawk
            if t >= 0.0 && t <= 1.0 && len_n <= adj_radius {

                // CRMakeRefs with -1 -1 1 2
                cur_path_node = 0;
                t0 = cur_pos;
                t1 = cur_pos;
                t2 = if 1 >= path.len() {
                    goal_pos
                } else {
                    path[1].pos
                };

                t3 = if 2 >= path.len() {
                    goal_pos
                } else {
                    path[2].pos
                };
            } else {
                // CRMakeRefs with t2 overwritten
                t0 = if cur_path_node - 1 <= -1 {
                    cur_pos
                } else {
                    if cur_path_node - 1 >= path.len() as i32 {
                        goal_pos
                    } else {
                        path[(cur_path_node - 1) as usize].pos
                    }
                };

                t1 = if cur_path_node<= -1 {
                    cur_pos
                } else {
                    if cur_path_node >= path.len() as i32 {
                        goal_pos
                    } else {
                        path[(cur_path_node) as usize].pos
                    }
                };

                t3 = if cur_path_node + 2 <= -1 {
                    cur_pos
                } else {
                    if cur_path_node + 2 >= path.len() as i32 {
                        goal_pos
                    } else {
                        path[(cur_path_node + 2) as usize].pos
                    }
                };

                if t >= 0.0 && t <= 1.0 {
                    t2 = (d * (t * len_next_cur)).add(cur_vec);
                } else if t < 0.0 {
                    t2 = path[0].pos;
                } else {
                    t2 = path[1].pos;
                }
            }
        }
    }

    // "Normal Logic" - Jhawk
    for _iter in 0..max_num_iter {
        let cur_vec = if cur_path_node == -1 {
            cur_pos
        } else {
            if cur_path_node >= path.len() as i32 {
                goal_pos
            } else {
                path[cur_path_node as usize].pos
            }
        };

        let next_vec = t2;
        let mut _use: Point<3, f32> = Point::<3, f32>([0.0, 0.0, 0.0]);

        // Goal mode is the same as in p2: when true, we are done with waypoints and heading straight to the ship (we're close!)
        if goal_mode == true {
            let diff = goal_pos.sub(cur_pos);
            // Once we reach this point, stop iterating; we're at the ship!!
            if diff.length() < 20.0 {
                break;
            }
            _use = diff.normalized();
        } else if next_vec.two_d().dist(&cur_pos.two_d()) < 6.0 {
            // Check if we're almost at the end of the path (second to last node?)
            // Cast len as signed i32 cause rust crashes if length is -1 (no length at all) cause rust moment :)
            if cur_path_node < ((path.len() as i32 ) - 2) as i32 {
                cur_path_node += 1;

                // CRMakeRefs
                t0 = if cur_path_node - 1 <= -1 {
                    cur_pos
                } else {
                    if cur_path_node - 1 >= path.len() as i32 {
                        goal_pos
                    } else {
                        path[(cur_path_node - 1) as usize].pos
                    }
                };
                t1 = if cur_path_node <= -1 {
                    cur_pos
                } else {
                    if cur_path_node >= path.len() as i32 {
                        goal_pos
                    } else {
                        path[(cur_path_node) as usize].pos
                    }
                };
                t2 = if cur_path_node + 1<= -1 {
                    cur_pos
                } else {
                    if cur_path_node + 1 >= path.len() as i32 {
                        goal_pos
                    } else {
                        path[(cur_path_node + 1) as usize].pos
                    }
                };
                t3 = if cur_path_node + 2 <= -1 {
                    cur_pos
                } else {
                    if cur_path_node + 2 >= path.len() as i32 {
                        goal_pos
                    } else {
                        path[(cur_path_node + 2) as usize].pos
                    }
                };
                // SPLINEEEEEEEEEEEE
                let vel = cr_spline_tangent(0.0, t0, t1, t2, t3).normalized();
                _use = vel;
            } else {
                // If we're here, that means we're close to the end of the path
                goal_mode = true;
                let vel = cr_spline_tangent(1.0, t0, t1, t2, t3).normalized();
                _use = vel;
            }
        } else {
            // In jhawk code this is a separate function normVector(); hopefully this achieves the same thing
            let d = (next_vec - cur_vec).normalized();
            let len_next_cur = cur_vec.dist(&next_vec);

            let mut t = cur_pos.sub(cur_vec).dot(d) / len_next_cur;

            if t < 0.0 {
                t = 0.0;
            }
            if t > 1.0 {
                t = 1.0;
            }

            let cur_radius = if cur_path_node == -1 {
                10.0
            } else {
                if cur_path_node >= path.len() as i32 {
                    50.0
                } else {
                    path[cur_path_node as usize].r
                }
            };
            let next_radius = if cur_path_node + 1 >= path.len() as i32 {
                50.0
            } else {
                path[(cur_path_node+1) as usize].r
            };
            let mut adj_radius = (1.0-t)*cur_radius + t*next_radius;
            // Don't let adjRadius be 0 for math reasons?
            if adj_radius == 0.0 {
                adj_radius = 1.0;
            }

            let n_full = (d * (t * len_next_cur)).add(cur_vec).sub(cur_pos);
            let len_n = n_full.length();
            let n = n_full.normalized();

            let mut away_ratio = len_n / adj_radius;
            // Not sure what this check is for or why this value specifically, but ig we can trust jhawk
            if away_ratio < 0.3 {
                away_ratio = 0.0;
            }

            if away_ratio <= 2.0 || len_n <= 130.0 {
                let mut use_n = 1.0;
                if away_ratio <= 1.0 {
                    use_n = f32::max(0.0, away_ratio);
                }

                if t < 1.0 {
                    let vel = cr_spline_tangent(t, t0, t1, t2, t3).normalized();
                    _use = (vel * (1.0 - use_n)).add(n * use_n);
                    // Check horizontal distance for if it's negative?
                    if (_use[0] * d[0]) + (_use[2] * d[2]) <= 0.0 {
                        _use = d;
                    }
                } else {
                    // Cast len as signed i32 cause rust crashes if length is -1 (no length at all) cause rust moment :)
                    if cur_path_node < (path.len() as i32 - 2) as i32 {
                        cur_path_node += 1;
                        // CRMakeRefs
                        t0 = if cur_path_node -1 <= -1 {
                            cur_pos
                        } else {
                            if cur_path_node -1 >= path.len() as i32 {
                                goal_pos
                            } else {
                                path[(cur_path_node-1) as usize].pos
                            }
                        };
                        t1 = if cur_path_node <= -1 {
                            cur_pos
                        } else {
                            if cur_path_node >= path.len() as i32 {
                                goal_pos
                            } else {
                                path[(cur_path_node) as usize].pos
                            }
                        };
                        t2 = if cur_path_node + 1 <= -1 {
                            cur_pos
                        } else {
                            if cur_path_node + 1 >= path.len() as i32 {
                                goal_pos
                            } else {
                                path[(cur_path_node+ 1) as usize].pos
                            }
                        };
                        t3 = if cur_path_node + 2 <= -1 {
                            cur_pos
                        } else {
                            if cur_path_node + 2 >= path.len() as i32 {
                                goal_pos
                            } else {
                                path[(cur_path_node + 2) as usize].pos
                            }
                        };

                        let vel = cr_spline_tangent(0.0, t0, t1, t2, t3).normalized();
                        _use = vel;
                    } else {
                        goal_mode = true;
                        let vel = cr_spline_tangent(t, t0, t1, t2, t3).normalized();
                        _use = vel;
                    }
                }
            } else {
                // "use pure normal ?? Idk." - jhawk (if he doesn't know then I sure as hell don't)
                _use = n;
            }
        }
        // ALMOST DONE!
        cur_vel = cur_vel.add(_use.normalized() * 0.05);
        if cur_vel.length() > 1.0 {
            cur_vel = cur_vel.normalized();
        }
        cur_pos = cur_pos.add(cur_vel * speed);
        // Finally, add our calculated position of the path to the return chain of paths
        ret_path.push(cur_pos);
    }
    return ret_path
}

// Create spline tangent stuff (copied from jhawk)
fn cr_spline_tangent(
    d: f32,
    t0 : Point<3, f32>,
    t1 : Point<3, f32>,
    t2 : Point<3, f32>,
    t3 : Point<3, f32>,
) -> Point<3, f32> {
    let r0 = -1.5 * d * d + 2.0 * d - 0.5;
    let r1 = 4.5 * d * d - 5.0 * d;
    let r2 = 0.5 - 4.5 * d * d + 4.0 * d;
    let r3 = 1.5 * d * d - d;
    Point([r0 * t0[0] + r1 * t1[0] + r2 * t2[0] + r3 * t3[0],
        r0 * t0[1] + r1 * t1[1] + r2 * t2[1] + r3 * t3[1],
        r0 * t0[2] + r1 * t1[2] + r2 * t2[2] + r3 * t3[2]])

}
