use nom::{IResult, bytes::complete::is_not, character::complete::char, combinator::{value, opt}, sequence::{delimited, tuple}};

use super::CaveInfo;

pub(super) fn parse_caveinfo(caveinfo_txt: &str) -> IResult<&str, CaveInfo> {
    // Header section (lazy because it's not that important)
    let (rest, header_txt) = next_section(caveinfo_txt)?;
    let num_floors: u8 = || -> Option<u8> {
        header_txt
            .trim()
            .lines()
            .next()?
            .split_whitespace()
            .nth(2)?
            .parse::<u8>()
            .ok()
    }()
    .expect("Malformed CaveInfo file");

    unimplemented!()
}

fn next_section(caveinfo_txt: &str) -> IResult<&str, &str> {
    let (caveinfo_txt, _) = discard_line_comment(caveinfo_txt)?;
    curly_brackets(caveinfo_txt)
}

fn curly_brackets(input: &str) -> IResult<&str, &str> {
    delimited(char('{'), is_not("}"), char('}'))(input)
}

fn discard_line_comment(input: &str) -> IResult<&str, Option<()>> {
    opt(
        value(
        (),
     tuple((
                char('#'),
                is_not("\n"),
                char('\n')
            ))
        )
    )(input)
}
