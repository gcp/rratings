extern crate chrono;
extern crate glob;
extern crate indicatif;
extern crate pgn_reader;
extern crate roots;
extern crate zstd;

mod glicko;
mod glicko2;
mod playerdb;

use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader};
use std::str;
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
use glob::glob;
use indicatif::{ProgressBar, ProgressDrawTarget};
use pgn_reader::Outcome::{self, Decisive, Draw};
use pgn_reader::{Color, Reader, Skip, Visitor};
use playerdb::{RatingDB, StatsDB};

#[derive(Clone, Debug, PartialEq)]
enum TimeControl {
    Garbage,
    Bullet,
    Blitz,
    Rapid,
    Classical,
    Correspondence,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultUpdate {
    white: String,
    black: String,
    result: Option<Outcome>,
    date: String,
    time: String,
    datetime: DateTime<Utc>,
    rated: bool,
    speed: TimeControl,
    white_rating: i32,
    black_rating: i32,
}

impl ResultUpdate {
    fn new() -> ResultUpdate {
        ResultUpdate {
            white: String::default(),
            black: String::default(),
            result: None,
            date: String::default(),
            time: String::default(),
            datetime: Utc.timestamp(0, 0),
            rated: false,
            speed: TimeControl::Garbage,
            white_rating: 1500,
            black_rating: 1500,
        }
    }

    fn valid(&self) -> bool {
        self.rated && self.speed != TimeControl::Garbage && self.result.is_some()
    }

    fn useful(&self) -> bool {
        self.valid() && self.speed == TimeControl::Blitz
    }
}

impl<'pgn> Visitor<'pgn> for ResultUpdate {
    type Result = ResultUpdate;

    fn header(&mut self, key: &'pgn [u8], value: &'pgn [u8]) {
        let mut strvalue = str::from_utf8(value).unwrap().to_string();
        if key == b"White" {
            self.white = strvalue;
        } else if key == b"Black" {
            self.black = strvalue;
        } else if key == b"Result" {
            if value == b"1-0" {
                self.result = Some(Decisive {
                    winner: Color::White,
                })
            } else if value == b"0-1" {
                self.result = Some(Decisive {
                    winner: Color::Black,
                })
            } else if value == b"1/2-1/2" {
                self.result = Some(Draw)
            }
        } else if key == b"UTCDate" {
            self.date = strvalue;
        } else if key == b"UTCTime" {
            self.time = strvalue;
        } else if key == b"Event" {
            strvalue.make_ascii_lowercase();
            if strvalue.contains("unrated") {
                self.rated = false;
                panic!("lichess DB has no unrated games");
            } else {
                assert!(strvalue.contains("rated"));
                self.rated = true;
            }
            if strvalue.contains("blitz") {
                self.speed = TimeControl::Blitz;
            } else if strvalue.contains("rapid") {
                self.speed = TimeControl::Rapid;
            } else if strvalue.contains("classical") {
                self.speed = TimeControl::Classical;
            } else if strvalue.contains("standard") {
                // WTF is this
                self.speed = TimeControl::Classical;
            } else if strvalue.contains("ultrabullet") {
                self.speed = TimeControl::Garbage;
            } else if strvalue.contains("bullet") {
                self.speed = TimeControl::Bullet;
                assert!(!strvalue.contains("ultrabullet"));
            } else if strvalue.contains("correspondence") {
                self.speed = TimeControl::Correspondence;
            } else {
                assert!(self.speed == TimeControl::Garbage);
                // println!("{:?}", strvalue);
            }
        } else if key == b"WhiteElo" {
            self.white_rating = strvalue.parse::<i32>().unwrap();
        } else if key == b"BlackElo" {
            self.black_rating = strvalue.parse::<i32>().unwrap();
        }
    }

    fn end_headers(&mut self) -> Skip {
        let mut datestring = self.date.clone();
        datestring.push_str(" ");
        datestring.push_str(&self.time);
        self.datetime = Utc
            .datetime_from_str(&datestring, "%Y.%m.%d %H:%M:%S")
            .unwrap();
        Skip(true)
    }

    fn end_game(&mut self, _game: &'pgn [u8]) -> Self::Result {
        self.clone()
    }
}

fn process_game(pgn: &str, db: &mut RatingDB) {
    let mut visitor = ResultUpdate::new();
    let mut reader = Reader::new(&mut visitor, pgn.as_bytes());

    let update = reader.read_game();
    if let Some(update) = update {
        if update.useful() {
            db.update(update);
        }
    }
    //println!("{:?}", update);
}

pub struct ProgressBarRead<'a, R> {
    bar: &'a ProgressBar,
    read: R,
}

impl<'a, R: io::Read> ProgressBarRead<'a, R> {
    fn new(bar: &'a ProgressBar, read: R) -> ProgressBarRead<'a, R> {
        ProgressBarRead { bar, read }
    }
}

impl<'a, R: io::Read> io::Read for ProgressBarRead<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let inc = self.read.read(buf)?;
        self.bar.inc(inc as u64);
        Ok(inc)
    }
}

fn process_zstd_pgn(path: std::path::PathBuf, db: &mut RatingDB) -> io::Result<()> {
    println!("Processing {}", path.display());

    let input_size = std::fs::metadata(&path)?.len();
    let pb = ProgressBar::new(input_size);
    pb.set_draw_target(ProgressDrawTarget::stderr());

    let input_file = File::open(path)?;
    let pbr = ProgressBarRead::new(&pb, input_file);
    let decoder = zstd::Decoder::new(pbr)?;

    pb.set_style(
        indicatif::ProgressStyle::default_bar().template(
            "[{elapsed_precise}] {spinner} {wide_bar} {bytes}/{total_bytes} {msg} [{eta}]",
        ),
    );

    let f = BufReader::new(decoder);
    let mut pgn_buff = String::from("");
    let mut empty = 0;
    let mut counter: u64 = 0;

    for line in f.lines() {
        if line.is_err() {
            return line.map(|_| ());
        }
        let line = line.unwrap();
        if line.is_empty() {
            empty += 1;
        }
        if empty == 2 {
            process_game(&pgn_buff, db);
            empty = 0;
            pgn_buff.clear();

            counter += 1;
            if counter % 10000 == 0 {
                pb.tick();
                let players = db.player_count();
                pb.set_message(&format!("{} games, {} players", counter, players));
            }
        } else {
            pgn_buff.push_str(&line);
            pgn_buff.push_str("\n");
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    const BASEDIR: &str = "/srv/large/PGN/";
    const BASEPREFIX: &str = "lichess_db_standard_rated_";

    let input_glob = String::from(BASEDIR) + BASEPREFIX + "*.zst";

    let mut paths: Vec<_> = glob(&input_glob).unwrap().filter_map(Result::ok).collect();
    paths.sort();

    let mut db = RatingDB::new();

    for path in paths {
        process_zstd_pgn(path, &mut db)?;
        println!("{}", db.get_stats());
        db.stats_reset();
    }

    Ok(())
}
