use std::{cmp::Ordering, fmt::Display};
use itertools::Itertools;
use nom::{
    sequence::tuple, 
    character::{
        complete::{alpha1, u32 as nomU32, space1, space0},
    }, 
    branch::alt, bytes::complete::tag, multi::many1, combinator::recognize, IResult
};

use crate::{errors::SearchConditionError, layout::Layout, caveinfo::{RoomType, CaveUnit}, assets::ASSETS};

#[derive(Clone, Debug)]
pub struct Query {
    pub search_conditions: Vec<SearchCondition>,
}

impl Query {
    pub fn matches(&self, layout: &Layout) -> bool {
        self.search_conditions.iter().all(|cond| cond.matches(layout))
    }
}

impl Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, cond) in self.search_conditions.iter().enumerate() {
            write!(f, "{}", cond)?;
            if i != self.search_conditions.len() - 1 {
                write!(f, " & ")?;
            }
        }
        Ok(())
    }
}

/// Programmatically defined conditions to search for in a sublevel
#[derive(Clone, Debug)]
pub enum SearchCondition {
    CountEntity{ name: String, relationship: Ordering, amount: usize },
    CountRoom{ room_matcher: RoomMatcher, relationship: Ordering, amount: usize },
    EntityInRoom{ entity_name: String, room_matcher: RoomMatcher },
    EntityInSameRoomAs{ entity1_name: String, entity2_name: String },
}

impl SearchCondition {
    pub fn matches(&self, layout: &Layout) -> bool {
        match self { 
            SearchCondition::CountEntity { name, relationship, amount } => {
                let entity_count = layout.get_spawn_objects()
                    .filter(|entity| entity.name().eq_ignore_ascii_case(name))
                    .count();
                &entity_count.cmp(&amount) == relationship
            },
            SearchCondition::CountRoom { room_matcher, relationship, amount } => {
                let unit_count = layout.map_units.iter()
                    .filter(|unit| room_matcher.matches(&unit.unit))
                    .count();
                &unit_count.cmp(&amount) == relationship
            },
            SearchCondition::EntityInRoom { entity_name, room_matcher } => {
                layout.map_units.iter()
                    .filter(|unit| room_matcher.matches(&unit.unit))
                    .any(|unit| {
                        unit.spawnpoints.iter()
                            .any(|sp| {
                                sp.contains.iter().any(|so| so.name().eq_ignore_ascii_case(entity_name))
                            })
                    })
            },
            SearchCondition::EntityInSameRoomAs { entity1_name, entity2_name } => {
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

impl TryFrom<&str> for Query {
    type Error = SearchConditionError;
    fn try_from(input: &str) -> Result<Self, Self::Error> {
        let mut remaining_text = input;
        let mut search_conditions = Vec::new();
        loop {
            if let Ok((rest, (obj, relationship_char, amount))) = compare_cmd(remaining_text) {
                remaining_text = rest;
                let relationship = char_to_ordering(relationship_char);
                if ASSETS.teki.contains(&obj.to_ascii_lowercase()) || obj.eq_ignore_ascii_case("gate") {
                    search_conditions.push(SearchCondition::CountEntity { name: obj.trim().to_string(), relationship, amount: amount as usize });
                }
                else {
                    search_conditions.push(SearchCondition::CountRoom { room_matcher: obj.trim().into(), relationship, amount: amount as usize });
                }
            }
            else if let Ok((rest, (entity, room))) = entity_in_room(remaining_text) {
                remaining_text = rest;
                search_conditions.push(SearchCondition::EntityInRoom { entity_name: entity.to_string(), room_matcher: room.into() });
            }
            else if let Ok((rest, (entity1, entity2))) = entity_in_same_room_as(remaining_text) {
                remaining_text = rest;
                search_conditions.push(SearchCondition::EntityInSameRoomAs { entity1_name: entity1.to_string(), entity2_name: entity2.to_string() });
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
        Ok(Query{ search_conditions })
    }
}

impl Display for SearchCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { 
            SearchCondition::CountEntity { name, relationship, amount } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>'
                };
                write!(f, "count {} {} {}", name, order_char, amount)
            },
            SearchCondition::CountRoom { room_matcher, relationship, amount } => {
                let order_char = match relationship {
                    Ordering::Less => '<',
                    Ordering::Equal => '=',
                    Ordering::Greater => '>'
                };
                write!(f, "count_entity {:?} {} {}", room_matcher, order_char, amount)
            },
            SearchCondition::EntityInRoom { entity_name, room_matcher } => {
                write!(f, "{} in {:?}", entity_name, room_matcher)
            },
            SearchCondition::EntityInSameRoomAs { entity1_name, entity2_name } => {
                write!(f, "{} with {}", entity1_name, entity2_name)
            }
        }
    }
}

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

// Parsing functions //

fn ident(input: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alpha1, tag("_"), tag("-")))))(input)
}

fn comparator(input: &str) -> IResult<&str, &str> {
    alt((tag("<"), tag("="), tag(">")))(input)
}

fn compare_cmd(input: &str) -> IResult<&str, (&str, &str, u32)> {
    let (rest, (name, _, relationship_char, _, amount)) = tuple((
        ident, space0, comparator, space0, nomU32,
    ))(input)?;
    Ok((rest, (name, relationship_char, amount)))
}

fn entity_in_room(input: &str) -> IResult<&str, (&str, &str)> {
    let (rest, (entity, _, _, _, room)) = tuple((
        ident, space1, tag("in"), space1, ident
    ))(input)?;
    Ok((rest, (entity, room)))
}

fn entity_in_same_room_as(input: &str) -> IResult<&str, (&str, &str)> {
    let (rest, (e1, _, _, _, e2)) = tuple((
        ident, space1, tag("with"), space1, ident
    ))(input)?;
    Ok((rest, (e1, e2)))
}

fn query_combinator(input: &str) -> IResult<&str, ()> {
    Ok((tuple((space0, tag("&"), space0))(input)?.0, ()))
}

fn char_to_ordering(c: &str) -> Ordering {
    match c {
        "<" => Ordering::Less,
        "=" => Ordering::Equal,
        ">" => Ordering::Greater,
        _ => panic!("Invalid comparison character!"),
    }
}
