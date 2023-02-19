use std::{borrow::Cow, io};

use lazy_static::lazy_static;

use regex::Regex;
use strum::{EnumIter, IntoEnumIterator};

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
        lang: &'a str,
        content: &'a str,
    },
}

fn extract_inner<'a>(tag_block: &'a str, kind: CodeKind) -> &'a str {
    // assume starts and ends have alredy been checked
    let tag_end = tag_block.find(']').expect("this can never happen") + 1;

    &tag_block[tag_end..(tag_block.len() - kind.end_seq().len())]
}

fn next_codestart<'a>(content: &'a str) -> Option<(usize, CodeKind)> {
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

fn next_codeend<'a>(content: &'a str, kind: CodeKind) -> Option<usize> {
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

fn parse_lang<'a>(content: &'a str) -> Option<&'a str> {
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

fn slurp_codetags<'a>(mut content: &'a str) -> Vec<TextChunk<'a>> {
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
                            lang,
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

fn convert_bbcode(content: &str) -> String {
    dbg!(slurp_codetags(content));

    String::new()
}

pub fn dump_markdown(mut writer: impl io::Write, content: &str) -> io::Result<()> {
    write!(writer, "{}", convert_bbcode(content))
}
