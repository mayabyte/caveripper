/// Parsing for CaveInfo files
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{
        alpha1, char, hex_digit1, line_ending, multispace0, none_of, not_line_ending, space1,
    },
    combinator::{into, map, not, opt, success, value},
    multi::{fill, many0, many1},
    number::complete::hex_u32,
    sequence::{delimited, preceded, tuple},
    IResult,
};

use super::CaveInfo;

pub(super) fn parse_caveinfo(caveinfo_txt: &str) -> IResult<&str, CaveInfo> {
    // Header section (lazy because it's not that important)
    let (rest, header_section) = section(caveinfo_txt)?;
    let num_floors: u32 = header_section
        .get_tagged_line("000")
        .and_then(|entries: &Vec<&str>| entries.get(1))
        .and_then(|num_floors_str| num_floors_str.parse().ok())
        .expect("Couldn't parse CaveInfo header section!");

    println!("{}", num_floors);

    unimplemented!()
}

struct InfoLine<'a> {
    tag: Option<&'a str>,
    items: Vec<&'a str>,
}

struct Section<'a> {
    lines: Vec<InfoLine<'a>>,
}

impl<'a> From<Vec<InfoLine<'a>>> for Section<'a> {
    fn from(vec_of_lines: Vec<InfoLine<'a>>) -> Self {
        Section {
            lines: vec_of_lines,
        }
    }
}

impl<'a> Section<'a> {
    pub fn get_tagged_line(&self, tag: &str) -> Option<&Vec<&'a str>> {
        self.lines
            .iter()
            .filter(|line| line.tag.contains(&tag))
            .next()
            .map(|line| &line.items)
    }
}

fn section(caveinfo_txt: &str) -> IResult<&str, Section> {
    let (caveinfo_txt, _) = line_comment(caveinfo_txt)?;
    into(delimited(char('{'), many1(info_line), char('}')))(caveinfo_txt)
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
