use std::{borrow::Cow, collections::HashSet, io, ops::ControlFlow};

use lazy_static::lazy_static;

use itertools::Itertools;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1, space1},
    combinator::{map, map_res, value},
    multi::fold_many0,
    sequence::{delimited, preceded, separated_pair},
    IResult,
};

use numerals::roman::Roman;
use regex::{Captures, Regex};
use strum::{EnumIter, IntoEnumIterator};

use crate::bbcode::write::{DEFAULT_ANON_CODELANG, DEFAULT_ANON_ICODELANG};

#[derive(Clone, Copy, Debug, EnumIter, Eq, PartialEq)]
enum CodeKind {
    Inline,
    Multiline,
}

impl CodeKind {
    const fn common_start() -> &'static str {
        "[c"
    }

    const fn end_seq(self) -> &'static str {
        use CodeKind::*;

        match self {
            Inline => "[/c]",
            Multiline => "[/code]",
        }
    }

    fn is_default_value(self, val: &str) -> bool {
        use CodeKind::*;

        val == match self {
            Inline => DEFAULT_ANON_ICODELANG,
            Multiline => DEFAULT_ANON_CODELANG,
        }
    }

    const fn start_seq(self) -> &'static str {
        use CodeKind::*;

        match self {
            Inline => "[c=",
            Multiline => "[code=",
        }
    }
}

#[derive(Debug)]
enum TextChunk<'a> {
    Chars(Cow<'a, str>),
    Code {
        kind: CodeKind,
        lang: Option<&'a str>,
        content: &'a str,
    },
}

fn extract_inner(tag_block: &str, kind: CodeKind) -> &str {
    // assume starts and ends have alredy been checked
    let tag_end = tag_block.find(']').expect("this can never happen") + 1;

    &tag_block[tag_end..(tag_block.len() - kind.end_seq().len())]
}

fn next_codestart(content: &str) -> Option<(usize, CodeKind)> {
    const PROBE: &str = CodeKind::common_start();

    content.find(PROBE).and_then(|pos| {
        let tag = &content[pos..];

        CodeKind::iter()
            .find_map(|bt| tag.starts_with(bt.start_seq()).then_some((pos, bt)))
            .or_else(|| {
                next_codestart(&tag[PROBE.len()..]).map(|(rel_pos, bt)| (pos + rel_pos, bt))
            })
    })
}

fn next_codeend(content: &str, kind: CodeKind) -> Option<usize> {
    const NEWLINES: &[u8] = b"\r\n";

    let probe = kind.end_seq().as_bytes();

    for i in 0..content.len() {
        let cur = content[i..].as_bytes();

        if cur.starts_with(probe) {
            return Some(i + probe.len());
        }

        // interpret the text as ASCII and check if the current position is a newline
        // if this is an inline code block this is illegal
        if kind == CodeKind::Inline && NEWLINES.contains(&cur[0]) {
            break;
        }
    }

    None
}

fn parse_lang(content: &str) -> Option<&str> {
    lazy_static! {
        static ref LANG_TAG: Regex = Regex::new(r#"^\s*"?([^"]+?)"?\s*\]"#).unwrap();
    }

    LANG_TAG
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
}

fn compact(chunks: Vec<TextChunk>) -> Vec<TextChunk> {
    use TextChunk::*;

    let (rem, mut ret) = chunks
        .into_iter()
        .fold((None, vec![]), |(current, mut acc), chunk| {
            let current = match (current, chunk) {
                (None, code @ Code { .. }) => {
                    acc.push(code);
                    None
                }
                (None, Chars(cs)) => Some(cs),
                (Some(text), code @ Code { .. }) => {
                    acc.extend([Chars(text), code]);
                    None
                }
                (Some(text), Chars(more)) => Some(format!("{text}{more}").into()),
            };

            (current, acc)
        });

    if let Some(reminder) = rem {
        ret.push(Chars(reminder));
    }

    ret
}

fn slurp_codetags(mut content: &str) -> Vec<TextChunk<'_>> {
    use TextChunk::*;

    let mut chunks = vec![];

    while let Some((at, kind)) = next_codestart(content) {
        let (before, start) = content.split_at(at);

        chunks.push(Chars(before.into()));

        let start_tok_len = kind.start_seq().len();
        let (tag, at_langstart) = start.split_at(start_tok_len);

        let (block, rem) = next_codeend(start, kind)
            .and_then(|pos| {
                let (code_block, rest) = start.split_at(pos);

                // skip the initial chunk and start with the `"`
                parse_lang(&code_block[start_tok_len..]).map(|lang| {
                    let inside = extract_inner(code_block, kind);

                    (
                        Code {
                            kind,
                            lang: if kind.is_default_value(lang) {
                                None
                            } else {
                                Some(lang)
                            },
                            content: inside,
                        },
                        rest,
                    )
                })
            })
            .unwrap_or_else(|| {
                // the first part of the tag becomes a char block, and we continue straight after it
                (Chars(tag.into()), at_langstart)
            });

        chunks.push(block);
        content = rem;
    }

    chunks.push(Chars(content.into()));

    compact(chunks)
}

fn code_str(kind: CodeKind, lang: Option<&str>, content: &str) -> String {
    use CodeKind::*;

    let lang = lang.unwrap_or_default();

    match kind {
        Inline => format!("`{content}`"),
        Multiline => format!("```{lang}\n{content}\n```\n",),
    }
}

struct ListHead {
    ltype: ListType,
    start: i16,
}

impl Default for ListHead {
    fn default() -> Self {
        Self {
            ltype: ListType::Unordered,
            start: 0i16,
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum ListType {
    Unordered,
    Ordered(NumberingStyle),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
enum NumberingStyle {
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

impl NumberingStyle {
    const fn is_upper(&self) -> bool {
        use NumberingStyle::*;

        matches!(self, UpperAlpha | UpperRoman)
    }

    fn iter_from(self, start: i16) -> impl Iterator<Item = String> {
        NumberingIterator::new(self, start)
    }
}

struct NumberingIterator {
    style: NumberingStyle,
    current: i16,
}

impl NumberingIterator {
    fn new(style: NumberingStyle, start: i16) -> Self {
        Self {
            style,
            current: start,
        }
    }
}

impl Iterator for NumberingIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        use NumberingStyle::*;

        let mut ret = match self.style {
            Decimal => self.current.to_string(),
            LowerAlpha | UpperAlpha => {
                let base = if self.style.is_upper() { b'A' } else { b'a' };

                char::from((self.current % 26) as u8 + base).to_string()
            }
            LowerRoman => format!("{:x}", Roman::from(self.current)),
            UpperRoman => format!("{:X}", Roman::from(self.current)),
        };

        ret.push_str(". ");

        self.current += 1;

        Some(ret)
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
enum ListHeadElement {
    Start(i16),
    Type(NumberingStyle),
}

fn list_head(input: &str) -> IResult<&str, ListHead> {
    use ListHeadElement::*;

    // use Nom to parse the list head - the language is actually not regular and requires a
    // bit of lookahead, so we can't use a single regex here. NERDZ uses several to achieve this
    // but it's undesirable due to the sheer amount of code repetition. Nom is faster and clearer TBH.

    let integer = map_res(digit1, str::parse);
    let start_spec = separated_pair(tag("start"), char('='), quoted(integer));
    let type_spec = separated_pair(tag("type"), char('='), quoted(ol_type));

    let (reminder, collected_tags) = fold_many0(
        preceded(
            space1,
            alt((
                map(start_spec, |(_, start)| Start(start)),
                map(type_spec, |(_, ty)| Type(ty)),
            )),
        ),
        HashSet::new,
        |mut acc, tag| {
            acc.insert(tag);
            acc
        },
    )(input)?;

    if !reminder.trim().is_empty() {
        use nom::{error::Error as NomError, error::ErrorKind as NomErrorKind, Err as NomErr};

        return Err(NomErr::Error(NomError::new(reminder, NomErrorKind::Tag)));
    }

    let head = collected_tags
        .into_iter()
        .fold(ListHead::default(), |mut head, tag| {
            match tag {
                Start(start) => {
                    head.start = start;

                    // if the list is unordered, make it ordered whenever a start is specified
                    if head.ltype == ListType::Unordered {
                        head.ltype = ListType::Ordered(NumberingStyle::Decimal);
                    }
                }
                Type(ty) => head.ltype = ListType::Ordered(ty),
            }

            head
        });

    Ok((reminder, head))
}

fn ol_type(input: &str) -> IResult<&str, NumberingStyle> {
    alt((
        value(NumberingStyle::Decimal, tag("1")),
        value(NumberingStyle::LowerAlpha, tag("a")),
        value(NumberingStyle::UpperAlpha, tag("A")),
        value(NumberingStyle::LowerRoman, tag("i")),
        value(NumberingStyle::UpperRoman, tag("I")),
    ))(input)
}

fn quoted<'a, O, E, F>(parser: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    E: nom::error::ParseError<&'a str>,
    F: nom::Parser<&'a str, O, E>,
{
    delimited(char('"'), parser, char('"'))
}

fn to_markdown_list(head: &str, content: &str) -> Option<String> {
    lazy_static! {
        static ref BBCODE_BULLET: Regex = Regex::new(r"\[\*\]\s*").unwrap();
    }

    let Ok(ListHead{ltype, start}) = list_head(head).map(|(_, lh)| lh) else {
        return None;
    };

    use ListType::*;

    Some(match ltype {
        Unordered => BBCODE_BULLET.replace_all(content, "- ").to_string(),
        Ordered(num) => {
            use ControlFlow::*;

            let processed = num
                .iter_from(start)
                .try_fold(content.to_string(), |current, it| {
                    let new = BBCODE_BULLET.replace(&current, &it);

                    match new != current {
                        true => Continue(new.to_string()),
                        false => Break(current),
                    }
                });

            match processed {
                Continue(s) | Break(s) => s,
            }
        }
    })
}

fn to_markdown_quote(text: &str) -> String {
    text.lines()
        .map(|line| format!("> {}", line))
        .intersperse_with(|| "\n".to_owned())
        .collect()
}

fn replace_bbcode(text: String) -> String {
    type ReplacerFn = fn(&Captures<'_>) -> String;
    lazy_static! {
        static ref REPLACEMENTS: [(Regex, ReplacerFn); 10] = [
            (
                Regex::new(r#"(?i)\[url="?(.+?)"?\](.+?)\[/url\]"#).unwrap(),
                |caps| format!("[{}]({})", &caps[2], &caps[1])
            ),
            (
                Regex::new(r#"(?i)\[url\](.+?)\[/url\]"#).unwrap(),
                |caps| format!("[]({})", &caps[1])
            ),
            (
                Regex::new(r#"(?mi)^[ \t]*\[big\](.+?)\[/big\][ \t]*$"#).unwrap(),
                |caps| format!("# {}", &caps[1])
            ),
            (
                Regex::new(r#"(?i)\[cur\](.+?)\[/cur\]"#).unwrap(),
                |caps| format!("*{}*", &caps[1])
            ),
            (
                Regex::new(r#"(?i)\[b\](.+?)\[/b\]"#).unwrap(),
                |caps| format!("**{}**", &caps[1])
            ),
            (
                Regex::new(r#"(?i)\[(?:i|cur)\](.+?)\[/(?:i|cur)\]"#).unwrap(),
                |caps| format!("*{}*", &caps[1])
            ),
            (
                Regex::new(r#"(?i)\[del\](.+?)\[/del\]"#).unwrap(),
                |caps| format!("~~{}~~", &caps[1])
            ),
            (
                Regex::new(r#"(?i)\[img\](.+?)\[/img\]"#).unwrap(),
                |caps| format!("![]({})", &caps[1])
            ),
            (
                Regex::new(r#"(?si)\[quote\](.+?)\[/quote\]"#).unwrap(),
                |caps| to_markdown_quote(&caps[1])
            ),
            (
                // parse a BBCode list with start= or type= attributes
                Regex::new(r#"(?si)\[list(.*?)\](.+?)\[/list\]"#).unwrap(),
                |caps| match to_markdown_list(&caps[1], &caps[2]) {
                    Some(s) => s,
                    None => caps[0].to_owned(),
                }
            )
        ];
    }

    REPLACEMENTS.iter().fold(text, |cur, (rx, repl)| {
        use Cow::*;

        match rx.replace_all(&cur, *repl) {
            Borrowed(_) => cur,
            Owned(new_string) => new_string,
        }
    })
}

fn convert_bbcode(content: &str) -> String {
    use TextChunk::*;

    slurp_codetags(content)
        .into_iter()
        .fold(String::new(), |mut s, blk| {
            let nxt = match blk {
                Chars(text) => replace_bbcode(text.into_owned()),
                Code {
                    kind,
                    lang,
                    content,
                } => code_str(kind, lang, content),
            };

            s.push_str(&nxt);

            s
        })
}

/// Writes the given content to the given writer, attempting to convert NERDZ BBCode to Markdown.
/// This function only supports a specific subset of NERDZ BBCode, especially the most "standard" bits such as
/// - `[b]P[/b]` -> **P**
/// - `[i]P[/i]` -> *P*
/// - `[cur]P[/cur]` -> *P*
/// - `[del]P[/del]` -> ~~P~~
/// - `<newline>[big]P[/big]<newline>` -> # P (header)
/// - `[url]P[/url]` -> [](P)
/// - `[url="P"]Q[/url]` -> [Q](P)
/// - `[img]P[/img]` -> ![](P)
/// - `[quote]P[/quote]` -> > P (multiline)
/// - `[list][*]P[/list]` -> - P (multiline)
/// - `[list type="a"][*]P[/list]` -> a. P (multiline, with optional `start="N"`)
/// - `[list type="A"][*]P[/list]` -> A. P (multiline, with optional `start="N"`)
/// - `[list type="i"][*]P[/list]` -> i. P (multiline, with optional `start="N"`)
/// - `[list type="I"][*]P[/list]` -> I. P (multiline, with optional `start="N"`)
/// - `[list start="N"][*]P[/list]` -> N. P (multiline, optionally with `type="1"`)
/// 
/// # Examples
///
/// ```
/// use std::{error::Error, io::{self, Write}, str};
/// use bibi::dump_markdown;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let mut writer = Vec::new();
///     dump_markdown(&mut writer, "[b]Hello[/b] [del]everybody[/del]")?;
///
///     // the BBCode replace engine doesn't add any newlines or has any notion of "paragraphs"
///     // in comparison to the Markdown parser
///     assert_eq!(str::from_utf8(&writer)?, "**Hello** ~~everybody~~");
///
///     Ok(())
/// } 
pub fn dump_markdown(mut writer: impl io::Write, content: &str) -> io::Result<()> {
    write!(writer, "{}", convert_bbcode(content))
}
