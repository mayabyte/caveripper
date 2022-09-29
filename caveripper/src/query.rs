#[cfg(test)]
mod test;

use std::{cmp::Ordering, fmt::Display, collections::{HashSet, HashMap}};
use itertools::Itertools;
use pest::{Parser, iterators::{Pair, Pairs}};
use pest_derive::Parser;
use crate::{
    errors::SearchConditionError,
    layout::{Layout, SpawnObject},
    caveinfo::{RoomType, CaveUnit},
    assets::AssetManager,
    sublevel::Sublevel,
    pikmin_math::dist,
};

#[derive(Parser)]
#[grammar = "query/query_grammar.pest"]
struct QueryParser;

#[derive(Clone, Debug)]
pub struct Query {
    pub clauses: Vec<QueryClause>,
}

impl Query {
    pub fn matches(&self, seed: u32) -> bool {
        let unique_sublevels: HashSet<&Sublevel> = self.clauses.iter().map(|clause| &clause.sublevel).collect();
        let layouts: HashMap<&Sublevel, Layout> = unique_sublevels.into_iter()
            .map(|sublevel| {
                let caveinfo = AssetManager::get_caveinfo(sublevel).unwrap();
                (sublevel, Layout::generate(seed, caveinfo))
            })
            .collect();
        self.clauses.iter().all(|clause| clause.matches(&layouts[&clause.sublevel]))
    }
}

/// Parse a series of SearchConditions from a query string, usually passed in by the CLI.
/// This effectively defines a DSL for search terms.
impl TryFrom<&str> for Query {
    type Error = SearchConditionError;
    fn try_from(input: &str) -> Result<Self, Self::Error> {
        let pairs = QueryParser::parse(Rule::query, input)
            .map_err(|e| SearchConditionError::ParseError(e.to_string()))?;
        let mut sublevel: Option<Sublevel> = None;
        let mut clauses = Vec::new();
        for pair in pairs {
            match pair.as_rule() {
                Rule::sublevel_ident => {
                    sublevel = Some(pair.as_str().try_into()?);
                },
                Rule::expression => {
                    if let Some(sublevel) = sublevel.as_ref() {
                        clauses.push(QueryClause{sublevel: sublevel.clone(), querykind: pair.try_into()?});
                    }
                    else {
                        return Err(SearchConditionError::ParseError("No sublevel provided".into()));
                    }
                },
                Rule::EOI => {}, // The end-of-input rule gets matched as an explicit token, so we have to ignore it.
                rule => return Err(SearchConditionError::ParseError(format!("Expected expression, got {:?} \"{}\"", rule, pair.as_str())))
            }
        }
        Ok(Query{clauses})
    }
}

impl Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, cond) in self.clauses.iter().enumerate() {
            write!(f, "{}", cond)?;
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
    CountEntity{ entity_matcher: EntityMatcher, relationship: Ordering, amount: usize },
    CountRoom{ unit_matcher: UnitMatcher, relationship: Ordering, amount: usize },
    StraightLineDist{ entity1: EntityMatcher, entity2: EntityMatcher, relationship: Ordering, req_dist: f32 },
    RoomPath(RoomPath),
}

impl QueryKind {
    /// Checks whether the given layout matches the query condition.
    pub fn matches<'a>(&self, layout: &'a Layout<'a>) -> bool {
        match self {
            QueryKind::CountEntity { entity_matcher, relationship, amount } => {
                let entity_count = layout.get_spawn_objects()
                    .filter(|entity| entity_matcher.matches(entity))
                    .count();
                entity_count.cmp(amount) == *relationship
            },
            QueryKind::CountRoom { unit_matcher: room_matcher, relationship, amount } => {
                let unit_count = layout.map_units.iter()
                    .filter(|unit| room_matcher.matches(unit.unit))
                    .count();
                unit_count.cmp(amount) == *relationship
            },
            QueryKind::StraightLineDist { entity1, entity2, relationship, req_dist } => {
                let e1s = layout.get_spawn_objects_with_position()
                    .filter(|(so, (_, _))| entity1.matches(so));
                let e2s = layout.get_spawn_objects_with_position()
                    .filter(|(so, (_, _))| entity2.matches(so));
                e1s.cartesian_product(e2s.collect_vec())
                    .any(|((_, (x1, z1)), (_, (x2, z2)))| {
                        let d = dist(x1, z1, x2, z2);
                        d.partial_cmp(req_dist).map(|ordering| ordering == *relationship).unwrap_or(false)
                    })
            },
            QueryKind::RoomPath(search_path) => search_path.matches(layout),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for QueryKind {
    type Error = SearchConditionError;
    fn try_from(input: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        if input.as_rule() != Rule::expression {
            return Err(SearchConditionError::ParseError(format!("Expected expression, got {}", input.as_str())));
        }

        let full_txt = input.as_str();
        let expr = input.into_inner().next().unwrap();
        match (expr.as_rule(), expr.into_inner()) {
            (Rule::compare, inner) => {
                let values: Vec<&str> = inner.map(|v| v.as_str().trim()).collect();
                let bare_name = values[0].find('/').map_or(values[0], |idx| &values[0][..idx]);
                if AssetManager::teki_list()?.contains(&bare_name.to_ascii_lowercase()) {
                    Ok(QueryKind::CountEntity {
                        entity_matcher: values[0].try_into()?,
                        relationship: char_to_ordering(values[1]),
                        amount: values[2].parse()?,
                    })
                }
                else if AssetManager::room_list()?.contains(&values[0].to_ascii_lowercase()) {
                    Ok(QueryKind::CountRoom {
                        unit_matcher: values[0].try_into()?,
                        relationship: char_to_ordering(values[1]),
                        amount: values[2].parse()?,
                    })
                }
                else {
                    Err(SearchConditionError::UnrecognizedName(values[0].into()))
                }
            },
            (Rule::straight_dist, inner) => {
                let values: Vec<&str> = inner.map(|v| v.as_str()).collect();
                Ok(QueryKind::StraightLineDist {
                    entity1: values[0].try_into()?,
                    entity2: values[1].try_into()?,
                    relationship: char_to_ordering(values[2]),
                    req_dist: values[3].parse()?,
                })
            },
            (Rule::room_path, inner) => Ok(QueryKind::RoomPath(inner.try_into()?)),
            _ => {
                Err(SearchConditionError::ParseError(format!("Couldn't parse query \"{}\"", full_txt)))
            }
        }
    }
}

impl Display for QueryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryKind::CountEntity { entity_matcher, relationship, amount } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>'
                };
                write!(f, "{} {} {}", entity_matcher, order_char, amount)
            },
            QueryKind::CountRoom { unit_matcher, relationship, amount } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>'
                };
                write!(f, "{} {} {}", unit_matcher, order_char, amount)
            },
            QueryKind::StraightLineDist { entity1, entity2, relationship, req_dist: dist } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>'
                };
                write!(f, "{} straight dist {} {} {}", entity1, entity2, order_char, dist)
            },
            QueryKind::RoomPath(room_path) => {
                let mut first = true;
                for (unit_matcher, entity_matchers) in room_path.components.iter() {
                    if !first {
                        write!(f, " -> ")?;
                    }
                    else {
                        first = false;
                    }

                    write!(f, "{}", unit_matcher)?;
                    for em in entity_matchers.iter() {
                        write!(f, " + {}", em)?;
                    }
                }
                Ok(())
            },
        }
    }
}

/// Matches a sequence of rooms connected in order, optionally with constraints on
/// the entities they must contain.
#[derive(Debug, Clone)]
pub struct RoomPath {
    components: Vec<(UnitMatcher, Vec<EntityMatcher>)>
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
                        let neighbors = unit.doors.iter()
                            .map(|door| door.borrow().adjacent_door.as_ref().unwrap().upgrade().unwrap().borrow().parent_idx.unwrap())
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

impl TryFrom<Pairs<'_, Rule>> for RoomPath {
    type Error = SearchConditionError;
    fn try_from(input: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
        let components = input.map(|component| -> Result<(UnitMatcher, Vec<EntityMatcher>), Self::Error> {
            let mut pairs = component.into_inner();
            Ok((
                pairs.next().unwrap().as_str().try_into()?,
                pairs.map(|e| e.as_str().try_into()).collect::<Result<Vec<_>, _>>()?
            ))
        })
        .collect::<Result<Vec<(UnitMatcher, Vec<EntityMatcher>)>, _>>()?;
        Ok(RoomPath{components})
    }
}

/// Matches entities or categories of entities.
#[derive(Debug, Clone)]
pub enum EntityMatcher {
    Teki{name: String, carrying: Option<String>},
    Treasure(String),
    Hole,
    Geyser,
    Ship,
    Gate,
}

impl EntityMatcher {
    fn matches(&self, spawn_object: &SpawnObject) -> bool {
        match (self, spawn_object) {
            (EntityMatcher::Teki{ name, carrying }, SpawnObject::Teki(tekiinfo, _)) => {
                let name_matches = name.eq_ignore_ascii_case("any") || name.eq_ignore_ascii_case(&tekiinfo.internal_name);
                let carrying_matches = carrying.as_ref().map_or(true,
                    |c1| tekiinfo.carrying.as_ref().map_or(c1.eq_ignore_ascii_case("any"),
                        |c2| c1.eq_ignore_ascii_case(&c2.internal_name))
                );
                name_matches && carrying_matches
            },
            // TODO: consolidate TekiInfo and CapInfo somehow so I don't need to double up code like this
            (EntityMatcher::Teki{ name, carrying }, SpawnObject::CapTeki(capinfo, _)) => {
                let name_matches = name.eq_ignore_ascii_case("any") || name.eq_ignore_ascii_case(&capinfo.internal_name);
                let carrying_matches = carrying.as_ref().map_or(true,
                    |c1| capinfo.carrying.as_ref().map_or(c1.eq_ignore_ascii_case("any"),
                        |c2| c1.eq_ignore_ascii_case(&c2.internal_name))
                );
                name_matches && carrying_matches
            },
            (EntityMatcher::Treasure(name), SpawnObject::Item(iteminfo)) => {
                name.eq_ignore_ascii_case("any") || name.eq_ignore_ascii_case(&iteminfo.internal_name)
            },
            (EntityMatcher::Hole, SpawnObject::Hole(_)) => true,
            (EntityMatcher::Geyser, SpawnObject::Geyser(_)) => true,
            (EntityMatcher::Ship, SpawnObject::Ship) => true,
            (EntityMatcher::Gate, SpawnObject::Gate(_)) => true,
            _ => false,
        }
    }
}

impl TryFrom<&str> for EntityMatcher {
    type Error = SearchConditionError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_ascii_lowercase().trim() {
            "hole" => Ok(EntityMatcher::Hole),
            "geyser" => Ok(EntityMatcher::Geyser),
            "ship" => Ok(EntityMatcher::Ship),
            "gate" => Ok(EntityMatcher::Gate),
            s => {
                if AssetManager::treasure_list()?.iter()
                    .map(|t| t.internal_name.trim())
                    .contains(&s)
                {
                    Ok(EntityMatcher::Treasure(s.to_string()))
                }
                else if s.contains('/') {
                    let (name, carrying) = s.split_once('/').unwrap();
                    if AssetManager::teki_list()?.contains(&name.to_string())
                        && AssetManager::treasure_list()?.iter().map(|t| t.internal_name.trim()).contains(&carrying)
                    {
                        Ok(EntityMatcher::Teki { name: name.trim().to_string(), carrying: Some(carrying.trim().to_string()) })
                    }
                    else {
                        Err(SearchConditionError::UnrecognizedEntityName(s.into()))
                    }
                }
                else if AssetManager::teki_list()?.contains(&s.to_string()) {
                    Ok(EntityMatcher::Teki { name: s.to_string(), carrying: None })
                }
                else {
                    Err(SearchConditionError::UnrecognizedEntityName(s.into()))
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
            EntityMatcher::Treasure(name) | EntityMatcher::Teki{name, carrying: None} => write!(f, "{}", name),
            EntityMatcher::Teki{name, carrying: Some(carrying)} => write!(f, "{}/{}", name, carrying),
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

impl TryFrom<&str> for UnitMatcher {
    type Error = SearchConditionError;
    fn try_from(input: &str) -> Result<Self, Self::Error> {
        if let Ok(room_type) = RoomType::try_from(input) {
            Ok(UnitMatcher::UnitType(room_type))
        }
        else if AssetManager::room_list()?.contains(&input.to_string()) || input.eq_ignore_ascii_case("any") {
            Ok(UnitMatcher::Named(input.to_string()))
        }
        else {
            Err(SearchConditionError::UnrecognizedUnitName(input.to_string()))
        }
    }
}

impl Display for UnitMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitMatcher::UnitType(t) => write!(f, "{}", t),
            UnitMatcher::Named(name) if name.eq_ignore_ascii_case("any") => write!(f, "any(room)"),
            UnitMatcher::Named(name) => write!(f, "{}", name)
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

/// A help string for query syntax. Placed here so it's usable by library consumers.
pub const QUERY_HELP: &str =
r##"A string with one or more queries, joined by '&'. Caveripper will attempt
to find a layout matching all queries. At least the first query must start with
a sublevel, and any further queries can specify different sublevels to check
complex conditions. If not specified, each query will use the most recently
specified sublevel.

Currently available query conditions:
- "INTERNAL_NAME </=/> NUM". Checks the number of the named entity present in
  each layout. This can include Teki, Treasures, Gates, "hole", "geyser", "ship",
  the internal name of a room tile, "alcove", "hallway", or "room".
  Example: "BlackPom > 0" to check for layouts that have at least one Violet
  Candypop Bud.
- "INTERNAL_NAME straight dist INTERNAL_NAME </=/> NUM". Checks whether the
  straight-line distance between the two named entities matches the specified
  value. Note that this is distance 'as the crow flies' rather than distance
  along carry paths.
- "ROOM_NAME (+ ENTITY_NAME / CARRYING)* -> <repeated>". This is a 'room path'
  query where you can specify a chain of rooms that all must be connected to
  each other, each optionally containing specific entities. The room and entity
  names here accept the word "any" as a special case. This query has a lot of
  uses, so here are some illustrative examples:
  - "bk4 room + hole": finds a layout where the hole is in a room.
  - "sh6 any + ship -> any + bluekochappy/bey_goma": finds a layout where the
    lens bulborb is in a room next to the ship.
  - "fc6 room_north4_1_tsuchi + chess_king_white + chess_queen_black": finds a
    fc6 layout where the two treasures are in the small round room.
  - "scx8 any + ship -> alcove + geyser": finds a layout where the geyser is
    in an alcove immediately next to the ship.
"##;
