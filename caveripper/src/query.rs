use std::{cmp::Ordering, fmt::Display};
use itertools::Itertools;
use nom::{
    sequence::tuple, 
    character::{
        complete::{u32 as nomU32, space1, space0, alphanumeric1},
    }, 
    branch::alt, bytes::complete::tag, multi::many1, combinator::recognize, IResult
};

use crate::{errors::SearchConditionError, layout::Layout, caveinfo::{RoomType, CaveUnit}, assets::ASSETS, sublevel::Sublevel};

#[derive(Clone, Debug)]
pub struct Query {
    pub clauses: Vec<QueryClause>,
}

impl Query {
    pub fn matches(&self, seed: u32) -> bool {
        self.clauses.iter().all(|cond| cond.matches(seed))
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
    pub fn matches(&self, seed: u32) -> bool {
        let caveinfo = ASSETS.get_caveinfo(&self.sublevel).unwrap();
        let layout = Layout::generate(seed, &caveinfo);
        self.querykind.matches(&layout)
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
    CountEntity{ name: String, relationship: Ordering, amount: usize },
    CountRoom{ room_matcher: RoomMatcher, relationship: Ordering, amount: usize },
    EntityInRoom{ entity_name: String, room_matcher: RoomMatcher },
    EntityInSameRoomAs{ entity1_name: String, entity2_name: String },
}

impl QueryKind {
    /// Checks whether the given layout matches the query condition.
    pub fn matches(&self, layout: &Layout) -> bool {
        match self { 
            QueryKind::CountEntity { name, relationship, amount } => {
                let entity_count = layout.get_spawn_objects()
                    .filter(|entity| entity.name().eq_ignore_ascii_case(name))
                    .count();
                entity_count.cmp(amount) == *relationship
            },
            QueryKind::CountRoom { room_matcher, relationship, amount } => {
                let unit_count = layout.map_units.iter()
                    .filter(|unit| room_matcher.matches(&unit.unit))
                    .count();
                unit_count.cmp(amount) == *relationship
            },
            QueryKind::EntityInRoom { entity_name, room_matcher } => {
                layout.map_units.iter()
                    .filter(|unit| room_matcher.matches(&unit.unit))
                    .any(|unit| {
                        unit.spawnpoints.iter()
                            .any(|sp| {
                                sp.contains.iter().any(|so| so.name().eq_ignore_ascii_case(entity_name))
                            })
                    })
            },
            QueryKind::EntityInSameRoomAs { entity1_name, entity2_name } => {
                let e1lower = entity1_name.to_ascii_lowercase();
                let e2lower = entity2_name.to_ascii_lowercase();
                layout.map_units.iter()
                    .any(|unit| {
                        let entities = unit.spawnpoints.iter()
                            .flat_map(|sp| sp.contains.iter().map(|e| e.name().to_ascii_lowercase()))
                            .collect_vec();
                        entities.contains(&e1lower) && entities.contains(&e2lower)
                    })
            }
        }
    }
}

/// Parse a series of SearchConditions from a query string, usually passed in by the CLI.
/// This effectively defines a DSL for search terms.
impl TryFrom<&str> for Query {
    type Error = SearchConditionError;
    fn try_from(input: &str) -> Result<Self, Self::Error> {
        let mut remaining_text = input;
        let mut clauses = Vec::new();
        loop {
            let sublevel;
            let rest;
            if let Ok((r, sublevel_str)) = ident_s(remaining_text) {
                sublevel = Sublevel::try_from(sublevel_str)
                    .map_err(|e| SearchConditionError::ParseError(e.to_string()))?;
                rest = r;
            }
            else {
                return Err(SearchConditionError::ParseError("No valid sublevel specified in query.".to_string()))
            }

            if let Ok((rest, (obj, relationship_char, amount))) = compare_cmd(rest) {
                remaining_text = rest;
                let relationship = char_to_ordering(relationship_char);
                if ASSETS.teki.contains(&obj.to_ascii_lowercase()) 
                    || ASSETS.treasures.iter().map(|t| t.internal_name.as_str()).any(|t| t.eq_ignore_ascii_case(&obj)) 
                    || obj.eq_ignore_ascii_case("gate") 
                {
                    clauses.push(QueryClause {
                        sublevel, 
                        querykind: QueryKind::CountEntity { 
                            name: obj.trim().to_string(), 
                            relationship, 
                            amount: amount as usize 
                        }
                    });
                }
                else if ASSETS.rooms.contains(&obj.to_ascii_lowercase()) 
                    || ["room", "cap", "alcove", "hall", "hallway"].contains(&obj.to_ascii_lowercase().as_str()) 
                {
                    clauses.push(QueryClause {
                        sublevel,
                        querykind: QueryKind::CountRoom { 
                            room_matcher: obj.trim().into(), 
                            relationship,
                            amount: amount as usize 
                        }
                    });
                }
                else {
                    return Err(SearchConditionError::InvalidArgument(format!("'{}' does not match any known object.", obj)));
                }
            }
            else if let Ok((rest, (entity, room))) = entity_in_room(remaining_text) {
                remaining_text = rest;
                clauses.push(QueryClause {
                    sublevel,
                    querykind: QueryKind::EntityInRoom { 
                        entity_name: entity.to_string(),
                        room_matcher: room.into() 
                    }
                });
            }
            else if let Ok((rest, (entity1, entity2))) = entity_in_same_room_as(remaining_text) {
                remaining_text = rest;
                clauses.push(QueryClause {
                    sublevel, 
                    querykind: QueryKind::EntityInSameRoomAs { 
                        entity1_name: entity1.to_string(), 
                        entity2_name: entity2.to_string()
                    }
                });
            }
            else {
                return Err(SearchConditionError::InvalidArgument("Unrecognized query".to_string()));
            }

            if remaining_text.trim().len() == 0 {
                break;
            }

            // Error if there isn't a combinator between queries
            let (rest, _) = query_combinator(remaining_text)
                .map_err(|_| SearchConditionError::MissingCombinator)?;
            remaining_text = rest;
        }
        Ok(Query{ clauses })
    }
}

impl Display for QueryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { 
            QueryKind::CountEntity { name, relationship, amount } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>'
                };
                write!(f, "count {} {} {}", name, order_char, amount)
            },
            QueryKind::CountRoom { room_matcher, relationship, amount } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>'
                };
                write!(f, "count_entity {:?} {} {}", room_matcher, order_char, amount)
            },
            QueryKind::EntityInRoom { entity_name, room_matcher } => {
                write!(f, "{} in {:?}", entity_name, room_matcher)
            },
            QueryKind::EntityInSameRoomAs { entity1_name, entity2_name } => {
                write!(f, "{} with {}", entity1_name, entity2_name)
            }
        }
    }
}

/// Helper type for matching rooms (really any cave unit). There are several attributes
/// on each unit that we may want to match against such as type, name, etc., and this
/// type abstracts over them so they can be used interchangeably.
#[derive(Debug, Clone)]
pub enum RoomMatcher {
    UnitType(RoomType),
    Named(String),
}

impl RoomMatcher {
    fn matches(&self, unit: &CaveUnit) -> bool {
        match self {
            RoomMatcher::UnitType(t) => &unit.room_type == t,
            RoomMatcher::Named(name) => unit.unit_folder_name.eq_ignore_ascii_case(name)
        }
    }
}

impl From<&str> for RoomMatcher {
    fn from(input: &str) -> Self {
        if let Ok(room_type) = RoomType::try_from(input) {
            RoomMatcher::UnitType(room_type)
        }
        else {
            RoomMatcher::Named(input.to_string())
        }
    }
}

/// Parsing functions for layout query strings ///

/// Parses an identifier consisting of letters, numbers, underscores, and dashes.
fn ident(input: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alphanumeric1, tag("_"), tag("-")))))(input)
}

/// Recognizes '<', '=', or '>' for mathematical relationships.
fn comparator(input: &str) -> IResult<&str, &str> {
    alt((tag("<"), tag("="), tag(">")))(input)
}

/// Retrieves an ident surrounded by spaces
fn ident_s(input: &str) -> IResult<&str, &str> {
    let (rest, (_, id, _)) = tuple((space0, ident, space0))(input)?;
    Ok((rest, id))
}

/// Recognizes an ampersand '&' optionally padded on either side by spaces.
fn query_combinator(input: &str) -> IResult<&str, ()> {
    Ok((tuple((space0, tag("&"), space0))(input)?.0, ()))
}

/// Parses any comparison command of the structure "IDENT_1 COMPARATOR IDENT_2".
fn compare_cmd(input: &str) -> IResult<&str, (&str, &str, u32)> {
    let (rest, (name, _, relationship_char, _, amount)) = tuple((
        ident, space0, comparator, space0, nomU32,
    ))(input)?;
    Ok((rest, (name, relationship_char, amount)))
}

/// Parses an "in" command of the structure "ENTITY_IDENT in ROOM_IDENT".
fn entity_in_room(input: &str) -> IResult<&str, (&str, &str)> {
    let (rest, (entity, _, _, _, room)) = tuple((
        ident, space1, tag("in"), space1, ident
    ))(input)?;
    Ok((rest, (entity, room)))
}

/// Parses a "with" command of the structure "ENTITY_1 with ENTITY_2".
fn entity_in_same_room_as(input: &str) -> IResult<&str, (&str, &str)> {
    let (rest, (e1, _, _, _, e2)) = tuple((
        ident, space1, tag("with"), space1, ident
    ))(input)?;
    Ok((rest, (e1, e2)))
}

fn char_to_ordering(c: &str) -> Ordering {
    match c {
        "<" => Ordering::Less,
        "=" => Ordering::Equal,
        ">" => Ordering::Greater,
        _ => panic!("Invalid comparison character!"),
    }
}
