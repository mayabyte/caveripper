use float_ord::FloatOrd;
use itertools::Itertools;
use petgraph::{Graph, Direction, visit::EdgeRef, prelude::NodeIndex};
use crate::point::Point;

use super::{Layout, SpawnObject};

#[derive(Debug, Clone)]
pub struct WaypointGraph {
    graph: Graph<WaypointGraphNode, f32>,
}

impl WaypointGraph {
    pub(super) fn build(layout: &Layout) -> Self {
        let mut graph = Graph::<WaypointGraphNode, f32>::new();

        // Connect waypoints within map units
        let nodes_per_unit = layout.map_units.iter()
            .map(|map_unit| {
                let nodes = map_unit.unit.waypoints.iter()
                    .map(|wp| {
                        let node = graph.add_node(WaypointGraphNode {
                            dist_to_start: f32::MAX,
                            idx: NodeIndex::new(0),
                            visited: false,
                            pos: wp.pos + Point([(map_unit.x as f32 * 170.0), 0.0, (map_unit.z as f32 * 170.0)]),
                            r: wp.r,
                        });
                        graph[node].idx = node;
                        node
                    })
                    .collect_vec();

                for (wp, node) in map_unit.unit.waypoints.iter().zip(nodes.iter()) {
                    for link in wp.links.iter() {
                        graph.add_edge(*node, nodes[*link], graph[*node].p2_dist(&graph[nodes[*link]]));
                    }
                }

                nodes
            })
            .collect_vec();

        // Connect doors between map units
        for map_unit in layout.map_units.iter() {
            for door in map_unit.doors.iter() {
                let node = nodes_per_unit[door.borrow().parent_idx.unwrap()][door.borrow().door_unit.waypoint_index];
                let adj_door = door.borrow().adjacent_door.as_ref().unwrap().upgrade().unwrap();
                let adj_unit_idx = adj_door.borrow().parent_idx.unwrap();
                let adj_node = nodes_per_unit[adj_unit_idx][adj_door.borrow().door_unit.waypoint_index];
                graph.add_edge(node, adj_node, f32::MAX);
            }
        }

        // Find start point
        let start_location = layout.get_spawn_objects_with_position()
            .find(|so| matches!(so.0, SpawnObject::Ship))
            .unwrap().1;
        let start_wp = graph.node_indices()
            .min_by_key(|wp| {
                let wp = &graph[*wp];
                FloatOrd(wp.pos.p2_dist(&start_location))
            }).unwrap();
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

            let mut walker = graph.neighbors_directed(closest, Direction::Incoming).detach();
            while let Some((_, invert)) = walker.next(&graph) {
                if graph[invert].visited { continue; }
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
        let edges_to_remove = graph.edges_directed(start_wp, Direction::Outgoing)
            .map(|e| e.id())
            .collect_vec();
        edges_to_remove.into_iter()
            .for_each(|e| {graph.remove_edge(e);});

        Self { graph }
    }

    pub fn iter(&self) -> impl Iterator<Item=&WaypointGraphNode> {
        self.graph.node_weights()
    }

    /// The waypoint a carrier should take from this waypoint to get back to the ship
    pub fn backlink(&self, wp: &WaypointGraphNode) -> Option<&WaypointGraphNode> {
        self.graph.neighbors_directed(wp.idx, Direction::Outgoing)
            .next()
            .map(|idx| &self.graph[idx])
    }

    /// The full chain of waypoints that should be taken from the provided point to get back to the ship
    pub fn carry_path_wps(&self, pos: Point<3,f32>) -> impl Iterator<Item=&WaypointGraphNode> {
        let start_wp = self.iter()
            .flat_map(|wp| {
                // Get segments between each combination of two adjacent waypoints
                self.graph.neighbors_directed(wp.idx, Direction::Incoming)
                    .map(move |wp2| (wp, &self.graph[wp2]))
            })
            .map(|(wp1, wp2)| {
                // Find the point's distance to each line segment
                let len = wp1.pos.p2_dist(&wp2.pos);
                if len <= 0.0 {
                    return (wp1, f32::MAX);
                }

                let norm = (wp1.pos - wp2.pos).normalized();
                let t = norm.dot(pos - wp1.pos) / len;

                if t <= 0.0 {
                    (wp1, pos.p2_dist(&wp1.pos) - wp1.r)
                }
                else if t >= 1.0 {
                    (wp2, pos.p2_dist(&wp2.pos) - wp2.r)
                }
                else {
                    let wp = if pos.p2_dist(&wp1.pos) - wp1.r < pos.p2_dist(&wp2.pos) - wp2.r { wp1 }
                        else { wp2 };
                    (wp, ((norm * len * t) + wp1.pos - pos).p2_length() - ((1.0 - t) * wp1.r) - (t * wp2.r))
                }
            })
            .min_by_key(|(_wp, dist)| FloatOrd(*dist))
            .unwrap().0;

        let mut ret = vec![start_wp];
        while let Some(backlink) = self.backlink(ret.last().unwrap()) {
            ret.push(backlink);
            if ret[ret.len()-1].pos == ret[ret.len()-2].pos {
                ret.remove(ret.len()-2);
            }
        }
        ret.into_iter()
    }
}

#[derive(Debug, Clone)]
pub struct WaypointGraphNode {
    pub dist_to_start: f32,
    idx: NodeIndex,
    visited: bool,
    pub pos: Point<3,f32>,
    pub r: f32,
}

impl WaypointGraphNode {
    pub fn p2_dist(&self, other: &Self) -> f32 {
        self.pos.p2_dist(&other.pos)
    }
}
