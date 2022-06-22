/// Parsing for CaveInfo files
use super::*;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{
        alpha1, char, digit1, hex_digit1, line_ending, multispace0, multispace1, not_line_ending,
    },
    combinator::{into, opt, success, value},
    multi::{count, many1},
    sequence::{delimited, preceded, tuple},
    IResult,
};
use std::str::FromStr;

/// Takes the entire raw text of a CaveInfo file and parses it into a
/// CaveInfo struct, ready for passing to the generator.
pub(super) fn parse_caveinfo(caveinfo_txt: &str) -> IResult<&str, Vec<[Section; 5]>> {
    // Header section
    let (rest, header_section) = section(caveinfo_txt)?;
    let num_floors: u8 = header_section
        .get_tag("000")
        .expect("Couldn't parse number of floors in CaveInfo header!");

    // CaveInfo files have one unique line after the header section that
    // repeats the floor number before the #FloorInfo comment. This skips
    // that line.
    let (rest, _) = skip_lines(rest, 1)?;

    // Read the five sections for each floor in the cave.
    let (_, sections) = count(section, 5 * num_floors as usize)(rest)?;
    let (floor_chunks, remainder): (&[[Section; 5]], &[_]) = sections.as_chunks::<5>();
    assert_eq!(
        remainder.len(),
        0,
        "CaveInfo files need to have exactly 5 sections per sublevel."
    );

    Ok(("", floor_chunks.to_vec()))
}

pub(super) fn parse_cave_unit_definition(
    cave_unit_definition_txt: &str,
) -> IResult<&str, Vec<Section>> {
    // Skip the 5 comment lines at the top of each file
    let (cave_unit_definition_txt, ()) = skip_lines(cave_unit_definition_txt, 5)?;

    let (rest, (num_units_str, _, _)) =
        tuple((digit1, multispace1, line_comment))(cave_unit_definition_txt)?;
    let num_units: usize = num_units_str
        .parse()
        .expect("Couldn't parse num units from Cave Unit Definition File!");

    count(section, num_units)(rest)
}

pub(super) fn parse_cave_unit_layout_file(cave_unit_layout_file_txt: &str) -> IResult<&str, Vec<Section>> {
    // Skip the first line, which is just a comment containing "BaseGen file"
    let (cave_unit_layout_file_txt, ()) = skip_lines(cave_unit_layout_file_txt, 1)?;

    let (rest, (num_gens_str, _, _)) = tuple((digit1, multispace1, line_comment))(cave_unit_layout_file_txt)?;
    let num_gens = num_gens_str.parse().expect("Couldn't parse num gens from Cave Unit Layout File!");

    count(section, num_gens)(rest)
}

/// One 'section' enclosed by curly brackets in a CaveInfo file.
#[derive(Clone, Debug)]
pub(super) struct Section<'a> {
    pub lines: Vec<InfoLine<'a>>,
}

impl<'a> From<Vec<InfoLine<'a>>> for Section<'a> {
    fn from(vec_of_lines: Vec<InfoLine<'a>>) -> Self {
        Section {
            lines: vec_of_lines,
        }
    }
}

impl<'a> Section<'a> {
    pub(self) fn get_tagged_line(&self, tag: &str) -> Option<&Vec<&'a str>> {
        self.lines
            .iter()
            .filter(|line| line.tag.contains(&tag))
            .next()
            .map(|line| &line.items)
    }

    /// Gets and parses the one useful value out of a tagged CaveInfo line.
    /// See https://pikmintkb.com/wiki/Cave_generation_parameters#FloorInfo
    pub(super) fn get_tag<T: FromStr>(&self, tag: &str) -> Result<T, CaveInfoError> {
        self.get_tagged_line(tag)
            .ok_or_else(|| CaveInfoError::NoSuchTag(tag.to_string()))?
            .get(1)
            .ok_or_else(|| CaveInfoError::MalformedTagLine(tag.to_string()))?
            .parse()
            .map_err(|_| CaveInfoError::ParseValueError)
    }

    pub(super) fn get_line(&self, index: usize) -> Result<&InfoLine, CaveInfoError> {
        self.lines.get(index).ok_or(CaveInfoError::MalformedLine)
    }
}

/// Simple helper struct to make working with individual lines easier.
#[derive(Clone, Debug)]
pub(super) struct InfoLine<'a> {
    pub tag: Option<&'a str>,
    pub items: Vec<&'a str>,
}

impl InfoLine<'_> {
    pub fn get_line_item(&self, item: usize) -> Result<&str, CaveInfoError> {
        self.items
            .get(item)
            .copied()
            .ok_or(CaveInfoError::MalformedLine)
    }
}

// **********************************************
//    Parsing raw caveinfo text into Sections
// **********************************************

fn section(caveinfo_txt: &str) -> IResult<&str, Section> {
    let (caveinfo_txt, _) = line_comment(caveinfo_txt)?;
    into(delimited(char('{'), many1(info_line), tag("}\r\n")))(caveinfo_txt)
}

fn info_line(input: &str) -> IResult<&str, InfoLine> {
    let (rest, (_, tag, items, _)) = tuple((
        multispace0,
        opt(info_tag),
        alt((is_not("\r\n}"), success(""))),
        line_ending,
    ))(input)?;
    Ok((
        rest,
        InfoLine {
            tag,
            items: items.split_whitespace().collect(),
        },
    ))
}

fn info_tag(input: &str) -> IResult<&str, &str> {
    delimited(
        char('{'),
        alt((tag("_eof"), preceded(alpha1, hex_digit1))),
        char('}'),
    )(input)
}

fn line_comment(input: &str) -> IResult<&str, Option<()>> {
    opt(value((), tuple((char('#'), not_line_ending, line_ending))))(input)
}

fn skip_lines(input: &str, skip: usize) -> IResult<&str, ()> {
    value((), count(tuple((not_line_ending, line_ending)), skip))(input)
}

// **************************************************
//    Parsing Sections into main CaveInfo structs
// **************************************************

impl TryFrom<Vec<[Section<'_>; 5]>> for CaveInfo {
    type Error = CaveInfoError;
    fn try_from(raw_sections: Vec<[parse::Section<'_>; 5]>) -> Result<CaveInfo, CaveInfoError> {
        let num_floors = raw_sections.len() as u32;
        let mut floors = raw_sections
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<FloorInfo>, _>>()?;
        floors.last_mut().unwrap().is_final_floor = true;

        Ok(CaveInfo{ num_floors, floors })
    }
}

impl TryFrom<[parse::Section<'_>; 5]> for FloorInfo {
    type Error = CaveInfoError;
    fn try_from(raw_sections: [parse::Section<'_>; 5]) -> Result<FloorInfo, CaveInfoError> {
        let [floorinfo_section, tekiinfo_section, iteminfo_section, gateinfo_section, capinfo_section] =
            raw_sections;

        let cave_unit_definition_file_name: String = floorinfo_section.get_tag("008")?;
        let cave_unit_definition_path = format!("assets/units/{}", &cave_unit_definition_file_name);
        let cave_unit_definition_text = get_file_JIS(&cave_unit_definition_path)
            .ok_or(CaveInfoError::MissingFileError(cave_unit_definition_path))?;
        let (_, cave_unit_sections) = parse_cave_unit_definition(&cave_unit_definition_text)
            .expect("Couldn't parse Cave Unit Definition file!");

        Ok(FloorInfo {
            cave_name: None,
            sublevel: floorinfo_section.get_tag("000")?,
            max_main_objects: floorinfo_section.get_tag("002")?,
            max_treasures: floorinfo_section.get_tag("003")?,
            max_gates: floorinfo_section.get_tag("004")?,
            num_rooms: floorinfo_section.get_tag("005")?,
            corridor_probability: floorinfo_section.get_tag("006")?,
            cap_probability: floorinfo_section.get_tag::<f32>("014")? / 100f32,
            has_geyser: floorinfo_section.get_tag::<u8>("007")? > 0,
            exit_plugged: floorinfo_section.get_tag::<u8>("010")? > 0,
            cave_units: expand_rotations(
                sort_cave_units(
                    cave_unit_sections
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<Vec<_>, _>>()?
                )
            ),
            teki_info: tekiinfo_section.try_into()?,
            item_info: iteminfo_section.try_into()?,
            gate_info: gateinfo_section.try_into()?,
            cap_info: capinfo_section.try_into()?,
            is_final_floor: false,
        })
    }
}

impl TryFrom<parse::Section<'_>> for Vec<TekiInfo> {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<TekiInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(
                |(item_line, group_line)| -> Result<TekiInfo, CaveInfoError> {
                    let internal_identifier = item_line.get_line_item(0)?;
                    let amount_code = item_line.get_line_item(1)?;
                    let group: u32 = group_line.get_line_item(0)?.parse()?;

                    let (spawn_method, internal_name, carrying) =
                        extract_internal_identifier(internal_identifier);

                    // Determine amount and filler_distribution_weight based on teki type
                    let minimum_amount: u32;
                    let filler_distribution_weight: u32;
                    if group == 6 {
                        // 6 is the group number for decorative teki
                        minimum_amount = amount_code.parse()?;
                        filler_distribution_weight = 0;
                    } else {
                        let (minimum_amount_str, filler_distribution_weight_str) =
                            amount_code.split_at(amount_code.len() - 1);

                        // If there is only one digit, it represents the filler_distribution_weight
                        // and minimum_amount defaults to 0.
                        minimum_amount = minimum_amount_str.parse().unwrap_or(0);
                        filler_distribution_weight = filler_distribution_weight_str.parse()?;
                    }

                    Ok(TekiInfo {
                        internal_name,
                        carrying,
                        minimum_amount,
                        filler_distribution_weight,
                        group,
                        spawn_method,
                    })
                },
            )
            .collect()
    }
}

impl TryFrom<parse::Section<'_>> for Vec<ItemInfo> {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<ItemInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1)
            .map(|line| -> Result<ItemInfo, CaveInfoError> {
                let amount_code_str = line.get_line_item(1)?;
                let (min_amount_str, filler_distribution_weight_str) =
                    amount_code_str.split_at(amount_code_str.len() - 1);
                Ok(ItemInfo {
                    internal_name: line.get_line_item(0)?.to_string(),
                    min_amount: min_amount_str.parse()?,
                    filler_distribution_weight: filler_distribution_weight_str.parse()?,
                })
            })
            .collect()
    }
}

impl TryFrom<parse::Section<'_>> for Vec<GateInfo> {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<GateInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1)
            .tuples()
            .map(
                |(health_line, spawn_distribution_weight_line)| -> Result<GateInfo, CaveInfoError> {
                    Ok(GateInfo {
                        health: health_line.get_line_item(1)?.parse()?,
                        spawn_distribution_weight: spawn_distribution_weight_line
                            .get_line_item(0)?
                            .chars()
                            .last()
                            .ok_or(CaveInfoError::MalformedLine)?
                            .to_digit(10)
                            .ok_or(CaveInfoError::ParseValueError)?
                            as u32,
                    })
                },
            )
            .collect()
    }
}

impl TryFrom<parse::Section<'_>> for Vec<CapInfo> {
    /// Almost an exact duplicate of the code for TekiInfo, which is unfortunately
    /// necessary with how the code is currently structured. May refactor in the future.
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<Vec<CapInfo>, CaveInfoError> {
        section
            .lines
            .iter()
            .skip(1) // First line contains the number of Teki
            .tuples()
            .map(
                |(_, item_line, group_line)| -> Result<CapInfo, CaveInfoError> {
                    let internal_identifier = item_line.get_line_item(0)?;
                    let amount_code = item_line.get_line_item(1)?;
                    let group: u8 = group_line.get_line_item(0)?.parse()?;

                    let (spawn_method, internal_name, carrying) =
                        extract_internal_identifier(internal_identifier);

                    // Determine amount and filler_distribution_weight based on teki type
                    let (minimum_amount_str, filler_distribution_weight_str) =
                        amount_code.split_at(amount_code.len() - 1);
                    // If there is only one digit, it represents the filler_distribution_weight
                    // and minimum_amount defaults to 0.
                    let minimum_amount = minimum_amount_str.parse().unwrap_or(0);
                    let filler_distribution_weight = filler_distribution_weight_str.parse()?;

                    Ok(CapInfo {
                        internal_name,
                        carrying,
                        minimum_amount,
                        filler_distribution_weight,
                        group,
                        spawn_method,
                    })
                },
            )
            .collect()
    }
}

impl TryFrom<parse::Section<'_>> for CaveUnit {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<CaveUnit, CaveInfoError> {
        let unit_folder_name = section.get_line(1)?.get_line_item(0)?.to_string();
        let width = section.get_line(2)?.get_line_item(0)?.parse()?;
        let height = section.get_line(2)?.get_line_item(1)?.parse()?;
        let room_type = section
            .get_line(3)?
            .get_line_item(0)?
            .parse::<usize>()?
            .into();
        let num_doors = section.get_line(5)?.get_line_item(0)?.parse()?;

        // DoorUnits
        let doors = if num_doors > 0 {
            let num_lines_per_door_unit = (section.lines.len() - 6) / num_doors;
            section.lines[6..]
                .chunks(num_lines_per_door_unit)
                .map(
                    |doorunit_lines: &[parse::InfoLine]| -> Result<DoorUnit, CaveInfoError> {
                        doorunit_lines.try_into()
                    },
                )
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };

        // Cave Unit Layout File (spawn points)
        let mut spawn_points = match get_file_JIS(&format!("assets/arc/{}/texts.d/layout.txt", unit_folder_name)) {
            Some(cave_unit_layout_file_txt) => {
                let spawn_points_sections = parse_cave_unit_layout_file(&cave_unit_layout_file_txt)
                    .expect("Couldn't parse cave unit layout file!").1;
                spawn_points_sections.into_iter().map(TryInto::try_into).collect::<Result<Vec<_>, _>>()?
            },
            None => Vec::new(),
        };

        // Add special Hole/Geyser spawnpoints to Cap and Hallway units. These aren't
        // present in Caveinfo files but the generation algorithm acts as if they're there,
        // so adding them here is a simplification.
        // Group 9 is a special group specifically for these 'fake' hole/geyser spawnpoints.
        // It does not appear in the game code or on the TKB.
        if (room_type == RoomType::DeadEnd && unit_folder_name.starts_with("item")) || room_type == RoomType::Hallway {
            spawn_points.push(
                SpawnPoint {
                    group: 9,
                    pos_x: 0.0,
                    pos_y: 0.0,
                    pos_z: 0.0,
                    angle_degrees: 0.0,
                    radius: 0.0,
                    min_num: 1,
                    max_num: 1
                }
            );
        }

        Ok(CaveUnit {
            unit_folder_name,
            width,
            height,
            room_type,
            num_doors,
            doors,
            rotation: 0,
            spawn_points,
        })
    }
}

impl TryFrom<&[parse::InfoLine<'_>]> for DoorUnit {
    type Error = CaveInfoError;
    fn try_from(lines: &[parse::InfoLine]) -> Result<DoorUnit, CaveInfoError> {
        let direction = lines[1].get_line_item(0)?.parse()?;
        let side_lateral_offset = lines[1].get_line_item(1)?.parse()?;
        let waypoint_index = lines[1].get_line_item(2)?.parse()?;
        let num_links = lines[2].get_line_item(0)?.parse()?;
        let door_links = lines[3..]
            .into_iter()
            .map(|line| line.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DoorUnit {
            direction,
            side_lateral_offset,
            waypoint_index,
            num_links,
            door_links,
        })
    }
}

impl TryFrom<&parse::InfoLine<'_>> for DoorLink {
    type Error = CaveInfoError;
    fn try_from(line: &parse::InfoLine) -> Result<DoorLink, CaveInfoError> {
        let distance = line.get_line_item(0)?.parse()?;
        let door_id = line.get_line_item(1)?.parse()?;
        let tekiflag = line.get_line_item(2)?.parse::<u8>()? > 0;
        Ok(DoorLink {
            distance,
            door_id,
            tekiflag,
        })
    }
}

impl TryFrom<parse::Section<'_>> for SpawnPoint {
    type Error = CaveInfoError;
    fn try_from(section: parse::Section) -> Result<SpawnPoint, Self::Error> {
        Ok(
            SpawnPoint {
                group: section.get_line(0)?.get_line_item(0)?.parse()?,
                pos_x: section.get_line(1)?.get_line_item(0)?.parse()?,
                pos_y: section.get_line(1)?.get_line_item(1)?.parse()?,
                pos_z: section.get_line(1)?.get_line_item(2)?.parse()?,
                angle_degrees: section.get_line(2)?.get_line_item(0)?.parse()?,
                radius: section.get_line(3)?.get_line_item(0)?.parse()?,
                min_num: section.get_line(4)?.get_line_item(0)?.parse()?,
                max_num: section.get_line(5)?.get_line_item(0)?.parse()?,
            }
        )
    }
}


// ************************
//    Utility Functions
// ************************

/// Retrieves Spawn Method, Internal Name, and Carrying Item from a combined
/// internal identifier as used by TekiInfo and CapInfo.
static INTERNAL_IDENTIFIER_RE: Lazy<Regex> = Lazy::new(|| {
    // Captures an optional Spawn Method and the Internal Name with the
    // Carrying item still attached.
    Regex::new(r"(\$\d?)?F?([A-Za-z_-]+)").unwrap()
});
fn extract_internal_identifier(
    internal_combined_name: &str,
) -> (Option<String>, String, Option<String>) {
    let captures = INTERNAL_IDENTIFIER_RE
        .captures(internal_combined_name)
        .expect(&format!(
            "Not able to capture info from combined internal identifier {}!",
            internal_combined_name
        ));
    let spawn_method = captures.get(1)
        .map(|s| s.as_str())
        .and_then(|sm| sm.strip_prefix('$'))
        .map(|s| s.to_string());
    let internal_combined_name = captures.get(2).unwrap().as_str().to_string();

    for treasure_name in TREASURES.lock().unwrap().iter() {
        if internal_combined_name.ends_with(&format!("_{}", treasure_name)) {
            return (
                spawn_method,
                internal_combined_name.strip_suffix(&format!("_{}", treasure_name)).unwrap().to_string(),
                Some(treasure_name.clone())
            );
        }
    }

    (spawn_method, internal_combined_name, None)
}