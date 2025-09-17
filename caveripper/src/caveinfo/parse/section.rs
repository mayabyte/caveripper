use std::{str::FromStr, error::Error, any::type_name};

use error_stack::{Result, report, ResultExt, bail, Report};
use pest::iterators::Pair;
use crate::caveinfo::{parse::Rule, error::CaveInfoError};

/// One 'section' enclosed by curly brackets in a CaveInfo file.
#[derive(Clone, Debug)]
pub struct Section<'a> {
    pub lines: Vec<InfoLine<'a>>,
}

impl<'a> TryFrom<Pair<'a, Rule>> for Section<'a> {
    type Error = Report<CaveInfoError>;
    fn try_from(pair: Pair<'a, Rule>) -> std::result::Result<Self, Self::Error> {
        if pair.as_rule() != Rule::section {
            bail!(CaveInfoError::ParseSection)
        }
        let lines: Vec<InfoLine> = pair.into_inner()
            .map(|section_line| {
                let mut line_items = section_line.into_inner()
                    .map(|li| li.as_str())
                    .peekable();
                if let Some(item) = line_items.peek() && item.starts_with('{') && item.ends_with('}') {
                    InfoLine {
                        tag: line_items.next(),
                        items: line_items.collect(),
                    }
                }
                else {
                    InfoLine { tag: None, items: line_items.collect() }
                }
            })
            .filter(|line| line.tag.is_some() || !line.items.is_empty())
            .collect();
        Ok(Section { lines })
    }
}

impl<'a> Section<'a> {
    /// Gets and parses the one useful value out of a tagged CaveInfo line.
    /// See https://pikmintkb.com/wiki/Cave_generation_parameters#FloorInfo
    pub fn get_tag<T: FromStr>(&self, tag: &str) -> Result<T, CaveInfoError>
    where <T as FromStr>::Err: Error + Send + Sync + 'static
    {
        self.get_nth_tag(tag, 1)
    }

    pub fn get_nth_tag<T: FromStr>(&self, tag: &str, idx: usize) -> Result<T, CaveInfoError>
    where <T as FromStr>::Err: Error + Send + Sync + 'static
    {
        self.get_tagged_line(tag)?.get_line_item::<T>(idx)
    }

    fn get_tagged_line(&self, tag: &str) -> Result<&InfoLine<'_>, CaveInfoError> {
        self.lines.iter()
            .find(|line| line.tag.is_some_and(|t| t.eq_ignore_ascii_case(tag)))
            .ok_or(report!(CaveInfoError::MissingItem))
            .attach_printable_lazy(|| format!("tag {tag}"))
    }

    pub fn get_line(&self, index: usize) -> Result<&InfoLine<'_>, CaveInfoError> {
        self.lines.get(index)
            .ok_or(report!(CaveInfoError::MissingItem))
            .attach_printable_lazy(|| format!("line {index}"))
            .attach_printable_lazy(|| format!("in section {self:#?}"))
    }
}

/// Simple helper struct to make working with individual lines easier.
#[derive(Clone, Debug)]
pub struct InfoLine<'a> {
    pub tag: Option<&'a str>,
    pub items: Vec<&'a str>,
}

impl InfoLine<'_> {
    pub fn get_line_item<T: FromStr>(&self, idx: usize) -> Result<T, CaveInfoError>
    where <T as FromStr>::Err: Error + Send + Sync + 'static
    {
        let item = self.items.get(idx)
            .ok_or(report!(CaveInfoError::MissingItem))
            .attach_printable_lazy(|| format!("line item {idx}"))?;
        item.parse::<T>()
            // Some float values in New Year are incorrectly formatted as "5.6.0000"
            // and this is needed to support those. The game parses them correctly.
            .or_else(|err| {
                let Some(first_part) = item.rsplit_once('.') else {return Err(err)};
                first_part.0.parse()
            })
            .change_context(CaveInfoError::ParseValue)
            .attach_printable_lazy(|| format!("{item:?} -> {}", type_name::<T>()))
    }
}
