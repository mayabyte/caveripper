mod search;
pub mod special;

#[cfg(test)]
mod test;

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Display,
};

use error_stack::{report, Result, ResultExt};
use itertools::Itertools;
use pest::{
    iterators::{Pair, Pairs},
    Parser,
};
use pest_derive::Parser;
pub use search::find_matching_layouts_parallel;

use crate::{
    assets::AssetManager,
    caveinfo::{CapInfo, CaveUnit, RoomType, TekiInfo},
    errors::CaveripperError,
    layout::{Layout, SpawnObject},
    point::Point,
    sublevel::Sublevel,
};

#[derive(Parser)]
#[grammar = "query/query_grammar.pest"]
struct QueryParser;

pub trait Query {
    fn matches(&self, seed: u32, mgr: &impl AssetManager) -> bool;
}

#[derive(Clone, Debug)]
pub struct StructuralQuery {
    pub clauses: Vec<QueryClause>,
}

impl Query for StructuralQuery {
    fn matches(&self, seed: u32, mgr: &impl AssetManager) -> bool {
        let unique_sublevels: HashSet<&Sublevel> = self.clauses.iter().map(|clause| &clause.sublevel).collect();
        let layouts: HashMap<&Sublevel, Layout> = unique_sublevels
            .into_iter()
            .map(|sublevel| {
                let caveinfo = mgr.get_caveinfo(sublevel).unwrap();
                (sublevel, Layout::generate(seed, caveinfo))
            })
            .collect();
        self.clauses.iter().all(|clause| clause.matches(&layouts[&clause.sublevel]))
    }
}

impl StructuralQuery {
    /// Parse a series of SearchConditions from a query string, usually passed in by the CLI.
    /// This effectively defines a DSL for search terms.
    pub fn try_parse(input: &str, mgr: &impl AssetManager) -> Result<Self, CaveripperError> {
        let pairs = QueryParser::parse(Rule::query, input).change_context(CaveripperError::QueryParseError)?;
        let mut sublevel: Option<Sublevel> = None;
        let mut clauses = Vec::new();
        for pair in pairs {
            match pair.as_rule() {
                Rule::sublevel_ident => {
                    sublevel = Some(
                        Sublevel::try_from_str(pair.as_str(), mgr)
                            .change_context(CaveripperError::QueryParseError)
                            .attach_printable_lazy(|| pair.as_str().to_string())?,
                    );
                }
                Rule::expression => {
                    if let Some(sublevel) = sublevel.as_ref() {
                        clauses.push(QueryClause {
                            sublevel: sublevel.clone(),
                            querykind: QueryKind::try_parse(pair, mgr)?,
                        });
                    } else {
                        return Err(report!(CaveripperError::QueryParseError));
                    }
                }
                Rule::EOI => {} // The end-of-input rule gets matched as an explicit token, so we have to ignore it.
                rule => {
                    return Err(report!(CaveripperError::QueryParseError)).attach_printable_lazy(|| format!("unexpected rule {rule:?}"))
                }
            }
        }
        Ok(StructuralQuery { clauses })
    }
}

impl Display for StructuralQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, cond) in self.clauses.iter().enumerate() {
            write!(f, "{cond}")?;
            if i != self.clauses.len() - 1 {
                write!(f, " & ")?;
            }
        }
        Ok(())
    }
}

/// A pairing of a sublevel with a single query statement.
#[derive(Clone, Debug)]
pub struct QueryClause {
    pub sublevel: Sublevel,
    pub querykind: QueryKind,
}

impl QueryClause {
    fn matches<'a>(&self, layout: &'a Layout<'a>) -> bool {
        self.querykind.matches(layout)
    }
}

impl Display for QueryClause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.sublevel.short_name(), self.querykind)
    }
}

/// Programmatically defined conditions to search for in a sublevel
#[derive(Clone, Debug)]
pub enum QueryKind {
    CountEntity {
        entity_matcher: EntityMatcher,
        relationship: Ordering,
        amount: usize,
    },
    CountRoom {
        unit_matcher: UnitMatcher,
        relationship: Ordering,
        amount: usize,
    },
    CarryDist {
        entity: EntityMatcher,
        relationship: Ordering,
        req_dist: f32,
    },
    StraightLineDist {
        entity1: EntityMatcher,
        entity2: EntityMatcher,
        relationship: Ordering,
        req_dist: f32,
    },
    Gated(EntityMatcher),
    NotGated(EntityMatcher),
    RoomPath(RoomPath),
}

impl QueryKind {
    /// Checks whether the given layout matches the query condition.
    pub fn matches<'a>(&self, layout: &'a Layout<'a>) -> bool {
        match self {
            QueryKind::CountEntity {
                entity_matcher,
                relationship,
                amount,
            } => {
                let entity_count = layout
                    .get_spawn_objects()
                    .filter(|(entity, _pos)| entity_matcher.matches(entity))
                    .count();
                entity_count.cmp(amount) == *relationship
            }
            QueryKind::CountRoom {
                unit_matcher: room_matcher,
                relationship,
                amount,
            } => {
                let unit_count = layout.map_units.iter().filter(|unit| room_matcher.matches(unit.unit)).count();
                unit_count.cmp(amount) == *relationship
            }
            QueryKind::CarryDist {
                entity,
                relationship,
                req_dist,
            } => layout
                .get_spawn_objects()
                .filter(|(so, _pos)| entity.matches(so))
                .map(|(_so, pos)| {
                    layout
                        .waypoint_graph()
                        .carry_path_wps(pos)
                        .tuple_windows()
                        .map(|(p1, p2)| p1.dist(&p2))
                        .sum::<f32>()
                })
                .any(|d| d.partial_cmp(req_dist).map(|ordering| ordering == *relationship).unwrap_or(false)),
            QueryKind::StraightLineDist {
                entity1,
                entity2,
                relationship,
                req_dist,
            } => {
                let e1s = layout.get_spawn_objects().filter(|(so, _)| entity1.matches(so));
                let e2s = layout.get_spawn_objects().filter(|(so, _)| entity2.matches(so));
                e1s.cartesian_product(e2s.collect_vec()).any(|((_, pos1), (_, pos2))| {
                    let d = pos1.p2_dist(&pos2);
                    d.partial_cmp(req_dist).map(|ordering| ordering == *relationship).unwrap_or(false)
                })
            }
            QueryKind::Gated(entity_matcher) => {
                let gates = layout
                    .get_spawn_objects()
                    .filter(|(so, _pos)| matches!(so, SpawnObject::Gate(_, _)))
                    .map(|(_so, pos)| pos)
                    .collect_vec();
                layout
                    .get_spawn_objects()
                    .filter(|(so, _pos)| entity_matcher.matches(so))
                    .any(|(_so, pos)| {
                        layout
                            .waypoint_graph()
                            .carry_path_wps(pos)
                            .tuple_windows()
                            .any(|(p1, p2)| gates.iter().any(|gate_pos| point_to_line_dist(*gate_pos, p1, p2) < 80.0))
                    })
            }
            // TODO: "not" operator in queries
            QueryKind::NotGated(entity_matcher) => {
                let gates = layout
                    .get_spawn_objects()
                    .filter(|(so, _pos)| matches!(so, SpawnObject::Gate(_, _)))
                    .map(|(_so, pos)| pos)
                    .collect_vec();
                layout
                    .get_spawn_objects()
                    .filter(|(so, _pos)| entity_matcher.matches(so))
                    .all(|(_so, pos)| {
                        layout
                            .waypoint_graph()
                            .carry_path_wps(pos)
                            .tuple_windows()
                            .all(|(p1, p2)| gates.iter().all(|gate_pos| point_to_line_dist(*gate_pos, p1, p2) > 80.0))
                    })
            }
            QueryKind::RoomPath(search_path) => search_path.matches(layout),
        }
    }

    pub fn try_parse(input: Pair<'_, Rule>, mgr: &impl AssetManager) -> Result<Self, CaveripperError> {
        if input.as_rule() != Rule::expression {
            return Err(report!(CaveripperError::QueryParseError)).attach_printable_lazy(|| input.as_str().to_string());
        }

        let full_txt = input.as_str().to_string();
        let expr = input.into_inner().next().unwrap();
        match (expr.as_rule(), expr.into_inner()) {
            (Rule::compare, inner) => {
                let values: Vec<&str> = inner.map(|v| v.as_str().trim()).collect();
                let bare_name = values[0].find('/').map_or(values[0], |idx| &values[0][..idx]);
                let bare_name_lowercase = bare_name.to_ascii_lowercase();

                let teki_list = mgr.combined_teki_list().change_context(CaveripperError::QueryParseError)?;
                let treasure_list = mgr.combined_treasure_list().change_context(CaveripperError::QueryParseError)?;
                let room_list = mgr.combined_room_list().change_context(CaveripperError::QueryParseError)?;

                if teki_list.contains(&bare_name_lowercase)
                    || treasure_list.iter().any(|t| t.internal_name.eq_ignore_ascii_case(bare_name))
                    || ["hole", "geyser", "ship", "gate"].contains(&bare_name_lowercase.as_str())
                {
                    Ok(QueryKind::CountEntity {
                        entity_matcher: values[0].into(),
                        relationship: char_to_ordering(values[1]),
                        amount: values[2].parse::<usize>().change_context(CaveripperError::QueryParseError)?,
                    })
                } else if room_list.contains(&bare_name_lowercase) || RoomType::try_from(values[0]).is_ok() {
                    Ok(QueryKind::CountRoom {
                        unit_matcher: values[0].into(),
                        relationship: char_to_ordering(values[1]),
                        amount: values[2].parse::<usize>().change_context(CaveripperError::QueryParseError)?,
                    })
                } else {
                    Err(report!(CaveripperError::QueryParseError)).attach_printable_lazy(|| full_txt.to_owned())
                }
            }
            (Rule::carry_dist, inner) => {
                let values: Vec<&str> = inner.map(|v| v.as_str()).collect();
                Ok(QueryKind::CarryDist {
                    entity: values[0].into(),
                    relationship: char_to_ordering(values[1]),
                    req_dist: values[2].parse::<f32>().change_context(CaveripperError::QueryParseError)?,
                })
            }
            (Rule::straight_dist, inner) => {
                let values: Vec<&str> = inner.map(|v| v.as_str()).collect();
                Ok(QueryKind::StraightLineDist {
                    entity1: values[0].into(),
                    entity2: values[1].into(),
                    relationship: char_to_ordering(values[2]),
                    req_dist: values[3].parse::<f32>().change_context(CaveripperError::QueryParseError)?,
                })
            }
            (Rule::gated, inner) => Ok(QueryKind::Gated(inner.as_str().into())),
            (Rule::not_gated, inner) => Ok(QueryKind::NotGated(inner.as_str().into())),
            (Rule::room_path, inner) => Ok(QueryKind::RoomPath(inner.into())),
            _ => Err(report!(CaveripperError::QueryParseError).attach_printable(full_txt)),
        }
    }
}

impl Display for QueryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryKind::CountEntity {
                entity_matcher,
                relationship,
                amount,
            } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>',
                };
                write!(f, "{entity_matcher} {order_char} {amount}")
            }
            QueryKind::CountRoom {
                unit_matcher,
                relationship,
                amount,
            } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>',
                };
                write!(f, "{unit_matcher} {order_char} {amount}")
            }
            QueryKind::CarryDist {
                entity,
                relationship,
                req_dist: dist,
            } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>',
                };
                write!(f, "{entity} carry dist {order_char} {dist}")
            }
            QueryKind::StraightLineDist {
                entity1,
                entity2,
                relationship,
                req_dist: dist,
            } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>',
                };
                write!(f, "{entity1} straight dist {entity2} {order_char} {dist}")
            }
            QueryKind::Gated(entity) => write!(f, "{entity} gated"),
            QueryKind::NotGated(entity) => write!(f, "{entity} not gated"),
            QueryKind::RoomPath(room_path) => {
                let mut first = true;
                for (unit_matcher, entity_matchers) in room_path.components.iter() {
                    if !first {
                        write!(f, " -> ")?;
                    } else {
                        first = false;
                    }

                    write!(f, "{unit_matcher}")?;
                    for em in entity_matchers.iter() {
                        write!(f, " + {em}")?;
                    }
                }
                Ok(())
            }
        }
    }
}

/// Matches a sequence of rooms connected in order, optionally with constraints on
/// the entities they must contain.
#[derive(Debug, Clone)]
pub struct RoomPath {
    components: Vec<(UnitMatcher, Vec<EntityMatcher>)>,
}

impl RoomPath {
    fn matches(&self, layout: &Layout) -> bool {
        layout.map_units.iter().any(|start_unit| {
            let mut frontier = vec![start_unit];
            let mut visited = Vec::new();
            for (unit_matcher, entity_matchers) in self.components.iter() {
                if frontier.is_empty() {
                    return false;
                }
                let mut new_frontier = Vec::new();
                let mut matched = false;
                for unit in frontier.iter() {
                    if visited.contains(&unit.key()) {
                        continue;
                    }
                    visited.push(unit.key());
                    if unit_matcher.matches(unit.unit) && entity_matchers.iter().all(|em| unit.spawn_objects().any(|so| em.matches(so))) {
                        matched = true;
                        let neighbors = unit
                            .doors
                            .iter()
                            .map(|door| {
                                door.borrow()
                                    .adjacent_door
                                    .as_ref()
                                    .unwrap()
                                    .upgrade()
                                    .unwrap()
                                    .borrow()
                                    .parent_idx
                                    .unwrap()
                            })
                            .map(|parent_idx| &layout.map_units[parent_idx])
                            .filter(|neighbor| neighbor.key() != unit.key());
                        new_frontier.extend(neighbors);
                    }
                }
                if !matched {
                    return false;
                }
                frontier = new_frontier;
            }
            true
        })
    }
}

impl From<Pairs<'_, Rule>> for RoomPath {
    fn from(input: Pairs<'_, Rule>) -> Self {
        let components = input
            .map(|component| {
                let mut pairs = component.into_inner();
                (pairs.next().unwrap().as_str().into(), pairs.map(|e| e.as_str().into()).collect())
            })
            .collect::<Vec<(UnitMatcher, Vec<EntityMatcher>)>>();
        RoomPath { components }
    }
}

/// Matches entities or categories of entities.
#[derive(Debug, Clone)]
pub enum EntityMatcher {
    Entity { name: String, carrying: Option<String> },
    Hole,
    Geyser,
    Ship,
    Gate,
}

impl EntityMatcher {
    fn matches(&self, spawn_object: &SpawnObject) -> bool {
        match (self, spawn_object) {
            (
                EntityMatcher::Entity { name, carrying },
                SpawnObject::Teki(
                    TekiInfo {
                        internal_name,
                        carrying: i_carrying,
                        ..
                    },
                    _,
                )
                | SpawnObject::CapTeki(
                    CapInfo {
                        internal_name,
                        carrying: i_carrying,
                        ..
                    },
                    _,
                ),
            ) => {
                let name_matches = name.eq_ignore_ascii_case("any") || name.eq_ignore_ascii_case(internal_name);
                let carrying_matches = carrying.as_ref().map_or(true, |c1| {
                    i_carrying
                        .as_ref()
                        .map_or(c1.eq_ignore_ascii_case("any"), |c2| c1.eq_ignore_ascii_case(c2))
                });
                name_matches && carrying_matches
            }
            (EntityMatcher::Entity { name, carrying }, SpawnObject::Item(iteminfo)) => {
                (name.eq_ignore_ascii_case("any") || name.eq_ignore_ascii_case(&iteminfo.internal_name)) && carrying.is_none()
            }
            (EntityMatcher::Hole, SpawnObject::Hole(_)) => true,
            (EntityMatcher::Geyser, SpawnObject::Geyser(_)) => true,
            (EntityMatcher::Ship, SpawnObject::Ship) => true,
            (EntityMatcher::Gate, SpawnObject::Gate(_, _)) => true,
            _ => false,
        }
    }
}

impl From<&str> for EntityMatcher {
    fn from(s: &str) -> Self {
        match s.to_ascii_lowercase().trim() {
            "hole" => EntityMatcher::Hole,
            "geyser" => EntityMatcher::Geyser,
            "ship" => EntityMatcher::Ship,
            "gate" => EntityMatcher::Gate,
            s => {
                if s.contains('/') {
                    let (name, carrying) = s.split_once('/').unwrap();
                    EntityMatcher::Entity {
                        name: name.trim().to_string(),
                        carrying: Some(carrying.trim().to_string()),
                    }
                } else {
                    EntityMatcher::Entity {
                        name: s.to_string(),
                        carrying: None,
                    }
                }
            }
        }
    }
}

impl Display for EntityMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityMatcher::Hole => write!(f, "hole"),
            EntityMatcher::Geyser => write!(f, "geyser"),
            EntityMatcher::Ship => write!(f, "ship"),
            EntityMatcher::Gate => write!(f, "gate"),
            EntityMatcher::Entity { name, carrying: None } => write!(f, "{name}"),
            EntityMatcher::Entity {
                name,
                carrying: Some(carrying),
            } => write!(f, "{name}/{carrying}"),
        }
    }
}

/// Matches cave units or types of cave units.
#[derive(Debug, Clone)]
pub enum UnitMatcher {
    UnitType(RoomType),
    Named(String),
}

impl UnitMatcher {
    fn matches(&self, unit: &CaveUnit) -> bool {
        match self {
            UnitMatcher::UnitType(t) => &unit.room_type == t,
            UnitMatcher::Named(name) if name.eq_ignore_ascii_case("any") => true,
            UnitMatcher::Named(name) => unit.unit_folder_name.eq_ignore_ascii_case(name),
        }
    }
}

impl From<&str> for UnitMatcher {
    fn from(input: &str) -> Self {
        if let Ok(room_type) = RoomType::try_from(input) {
            UnitMatcher::UnitType(room_type)
        } else {
            UnitMatcher::Named(input.to_string())
        }
    }
}

impl Display for UnitMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitMatcher::UnitType(t) => write!(f, "{t}"),
            UnitMatcher::Named(name) if name.eq_ignore_ascii_case("any") => write!(f, "any(room)"),
            UnitMatcher::Named(name) => write!(f, "{name}"),
        }
    }
}

fn char_to_ordering(c: &str) -> Ordering {
    match c {
        "<" => Ordering::Less,
        "=" => Ordering::Equal,
        ">" => Ordering::Greater,
        _ => panic!("Invalid comparison character!"),
    }
}

fn point_to_line_dist(p: Point<3, f32>, l1: Point<3, f32>, l2: Point<3, f32>) -> f32 {
    let len = l1.dist(&l2);
    if len <= 0.0 {
        return f32::MAX;
    }

    let norm = (l1 - l2).normalized();
    let t = norm.dot(p - l1) / len;

    if t <= 0.0 {
        p.dist(&l1)
    } else if t >= 1.0 {
        p.dist(&l2)
    } else {
        ((norm * len * t) + l1 - p).length()
    }
}
