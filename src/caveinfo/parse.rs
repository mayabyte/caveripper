/// Parsing for CaveInfo files

use std::str::FromStr;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, char, hex_digit1, line_ending, multispace0, not_line_ending},
    combinator::{into, opt, success, value},
    multi::{count, many1},
    sequence::{delimited, preceded, tuple},
    IResult,
};

/// Takes the entire raw text of a CaveInfo file and parses it into a
/// CaveInfo struct, ready for passing to the generator.
pub(super) fn parse_caveinfo<'c>(caveinfo_txt: &'c str) -> IResult<&str, Vec<[Section; 5]>> {
    // Header section
    let (rest, header_section) = section(caveinfo_txt)?;
    let num_floors: u8 = header_section
        .get_tag("000")
        .expect("Couldn't parse CaveInfo header section!");

    println!("{}", num_floors);

    // CaveInfo files have one unique line after the header section that
    // repeats the floor number before the #FloorInfo comment. This skips
    // that line.
    let (rest, _) = skip_line(rest)?;

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
    pub(super) fn get_tag<T: FromStr>(&self, tag: &str) -> Option<T> {
        self.get_tagged_line(tag)?.get(1)?.parse().ok()
    }
}

/// Simple helper struct to make working with individual lines easier.
#[derive(Clone, Debug)]
pub(super) struct InfoLine<'a> {
    pub tag: Option<&'a str>,
    pub items: Vec<&'a str>,
}

// *******************
//    Parsing Code
// *******************

fn section(caveinfo_txt: &str) -> IResult<&str, Section> {
    let (caveinfo_txt, _) = line_comment(caveinfo_txt)?;
    into(delimited(char('{'), many1(info_line), tag("}\n")))(caveinfo_txt)
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

fn skip_line(input: &str) -> IResult<&str, ()> {
    value((), tuple((not_line_ending, line_ending)))(input)
}
