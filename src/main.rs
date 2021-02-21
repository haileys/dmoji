use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use regex::Regex;

const EMOJI_SEQUENCES: &str = "emoji-sequences.txt";
const EMOJI_ZWJ_SEQUENCES: &str = "emoji-zwj-sequences.txt";

struct Emoji<'a> {
    pub sequence: Sequence,
    pub description: &'a str,
}

enum Sequence {
    Range(RangeInclusive<char>),
    Literal(String),
}

struct Scanner {
    line_re: Regex,
    range_re: Regex,
    lit_re: Regex,
}

impl Scanner {
    pub fn new() -> Self {
        let line_re = Regex::new(r"^(.*?);(.*?);(.*?)#(.*?)$").unwrap();
        let range_re = Regex::new(r"^([A-F0-9]+)\.\.([A-F0-9]+)$").unwrap();
        let lit_re = Regex::new(r"^[A-F0-9]+(\s+[A-F0-9])*$").unwrap();

        Scanner {
            line_re,
            range_re,
            lit_re,
        }
    }

    pub fn emoji<'a>(&'a self, text: &'a str) -> impl Iterator<Item = Emoji> + 'a {
        text.lines().flat_map(move |line| self.scan_line(line))
    }

    fn scan_line<'a>(&self, line: &'a str) -> Option<Emoji<'a>> {
        let line = self.line_re.captures(line)?;

        let sequence = self.scan_seq(line.get(1)?.as_str().trim())?;
        let description = line.get(3)?.as_str().trim();

        Some(Emoji {
            sequence,
            description,
        })
    }

    fn scan_seq(&self, seq: &str) -> Option<Sequence> {
        if let Some(range) = self.range_re.captures(seq) {
            let low = unichar(range.get(1)?.as_str())?;
            let high = unichar(range.get(2)?.as_str())?;

            return Some(Sequence::Range(low..=high));
        }

        if self.lit_re.is_match(seq) {
            let lit = seq
                .split_whitespace()
                .map(unichar)
                .collect::<Option<String>>()?;

            return Some(Sequence::Literal(lit))
        }

        return None;

        fn unichar(s: &str) -> Option<char> {
            Some(std::char::from_u32(u32::from_str_radix(s, 16).ok()?)?)
        }
    }
}

struct DataDir {
    path: PathBuf,
}

impl DataDir {
    pub fn locate() -> Self {
        // debug_assertions is the officially ordained cfg var to test for debug
        // builds: https://stackoverflow.com/a/39205417
        if cfg!(debug_assertions) {
            let mut exe = std::env::current_exe()
                .expect("std::env::current_exe");

            exe.pop();
            exe.pop();
            exe.pop();

            Self::new(exe)
        } else {
            if let Some(exe) = std::env::current_exe().ok() {
                let path = exe.join("../share/dmoji");

                if path.join(EMOJI_SEQUENCES).is_file() {
                    return Self::new(path);
                }
            }

            eprintln!("dmoji: no data dir found");
            std::process::exit(1);
        }
    }

    fn new(path: PathBuf) -> Self {
        DataDir { path }
    }

    pub fn load_file(&self, file: &str) -> String {
        let path = self.path.join(file);

        match std::fs::read_to_string(&path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("dmoji: could not read {}: {:?}", path.display(), e);
                String::new()
            }
        }
    }
}

fn main() {
    let scanner = Scanner::new();
    let data = DataDir::locate();

    // load emoji from data files
    let pri_emoji = data.load_file(EMOJI_SEQUENCES);
    let zwj_emoji = data.load_file(EMOJI_ZWJ_SEQUENCES);

    let emoji = scanner.emoji(&pri_emoji)
        .chain(scanner.emoji(&zwj_emoji));

    // construct map of description -> emoji
    let mut map = HashMap::new();

    for em in emoji {
        match em.sequence {
            Sequence::Literal(seq) => {
                map.insert(Cow::Borrowed(em.description), seq);
            }
            Sequence::Range(chars) => {
                for (idx, ch) in chars.enumerate() {
                    let name = format!("{}-{}", em.description, idx);
                    map.insert(Cow::Owned(name), ch.to_string());
                }
            }
        }
    }

    // spawn dmenu
    let menu_proc = Command::new("dmenu")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();

    let mut menu_proc = match menu_proc {
        Ok(prc) => prc,
        Err(e) => {
            eprintln!("dmoji: failed to spawn dmenu: {:?}", e);
            std::process::exit(1);
        }
    };

    // write emoji choices
    let mut dmenu_in = menu_proc.stdin.take().unwrap();

    for (descr, _) in &map {
        let _ = write!(dmenu_in, "{}\n", descr);
    }

    drop(dmenu_in);

    // read back selection
    let mut dmenu_out = menu_proc.stdout.take().unwrap();
    let mut selection = String::new();
    match dmenu_out.read_to_string(&mut selection) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("dmoji: error reading from dmenu: {:?}", e);
            std::process::exit(1);
        }
    }

    // look up selection
    let emoji = match map.get(selection.trim()) {
        Some(seq) => seq,
        None => {
            // not found
            std::process::exit(1);
        }
    };

    // spawn wl-copy
    let wl_copy_proc = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn();

    let mut wl_copy_proc = match wl_copy_proc {
        Ok(prc) => prc,
        Err(e) => {
            eprintln!("dmoji: failed to spawn wl-copy: {:?}", e);
            std::process::exit(1);
        }
    };

    // write emoji corresponding to selection to wl-copy
    let mut wl_copy_in = wl_copy_proc.stdin.take().unwrap();
    let _ = wl_copy_in.write_all(emoji.as_bytes());
}
