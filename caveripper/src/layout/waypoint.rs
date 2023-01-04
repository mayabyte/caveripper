use float_ord::FloatOrd;
use itertools::Itertools;
use petgraph::{Graph, Direction, visit::EdgeRef, prelude::NodeIndex};
use super::{Layout, SpawnObject};

#[derive(Debug, Clone)]
pub struct WaypointGraph {
    graph: Graph<WaypointGraphNode, f32>,
}

impl WaypointGraph {
    pub fn build(layout: &Layout) -> Self {
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
                            x: wp.x + (map_unit.x as f32 * 170.0),
                            y: wp.y,
                            z: wp.z + (map_unit.z as f32 * 170.0)
                        });
                        graph[node].idx = node;
                        node
                    })
                    .collect_vec();

                for (wp, node) in map_unit.unit.waypoints.iter().zip(nodes.iter()) {
                    for link in wp.links.iter() {
                        graph.add_edge(*node, nodes[*link], graph[*node].dist(&graph[nodes[*link]]));
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
                let dx = wp.x - start_location.0;
                let dz = wp.z - start_location.1;
                FloatOrd((dx.powi(2) + dz.powi(2)).sqrt())
            }).unwrap();
        graph[start_wp].dist_to_start = 0.0;

        // Expand the frontier, marking distances and backlinks along the way
        let mut frontier = vec![start_wp];
        while !frontier.is_empty() {
            frontier.sort_by_key(|wp| {
                let wp = &graph[*wp];
                let dx = wp.x - start_location.0;
                let dz = wp.z - start_location.1;
                FloatOrd((dx.powi(2) + dz.powi(2)).sqrt() * -1.0) // Sort backwards
            });
            let closest = frontier.pop().unwrap();
            graph[closest].visited = true;

            let mut walker = graph.neighbors_directed(closest, Direction::Incoming).detach();
            while let Some((_, invert)) = walker.next(&graph) {
                if graph[invert].visited { continue; }
                if graph[invert].dist_to_start == f32::MAX {
                    frontier.push(invert);
                }
                let dist = graph[closest].dist(&graph[invert]) + graph[closest].dist_to_start;
                if dist < graph[invert].dist_to_start {
                    // Delete all the outgoing edges from this node so the only one is the one
                    // towards the ship.
                    let edges_to_remove = graph.edges_directed(invert, Direction::Outgoing)
                        .map(|e| e.id())
                        .collect_vec();
                    edges_to_remove.into_iter()
                        .for_each(|e| {graph.remove_edge(e);});

                    graph.add_edge(invert, closest, graph[invert].dist(&graph[closest]));
                    graph[invert].dist_to_start = dist;
                }
            }
        }

        Self { graph }
    }

    pub fn iter(&self) -> impl Iterator<Item=&WaypointGraphNode> {
        self.graph.node_weights()
    }

    pub fn backlink(&self, wp: &WaypointGraphNode) -> Option<&WaypointGraphNode> {
        self.graph.neighbors_directed(wp.idx, Direction::Outgoing)
            .next()
            .map(|idx| &self.graph[idx])
    }
}

#[derive(Debug, Clone)]
pub struct WaypointGraphNode {
    pub dist_to_start: f32,
    idx: NodeIndex,
    visited: bool,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl WaypointGraphNode {
    pub fn dist(&self, other: &Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx.powi(2) + dy.powi(2) + dz.powi(2)).sqrt()
    }
}
