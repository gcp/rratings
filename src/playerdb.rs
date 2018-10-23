use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
use pgn_reader::Color;

use super::ResultUpdate;
use glicko::GlickoRating;

#[derive(Clone, Debug)]
pub struct Player {
    name: String,
    rating: GlickoRating,
    mtime: DateTime<Utc>,
}

impl Player {
    pub fn new(name: &str) -> Player {
        Player {
            name: name.to_string(),
            rating: GlickoRating::new(),
            mtime: Utc.timestamp(0, 0),
        }
    }
}

pub struct StatsDB {
    glicko_guess: AtomicUsize,
    glicko_predicted: AtomicUsize,
}

impl StatsDB {
    pub fn new() -> StatsDB {
        StatsDB {
            glicko_guess: AtomicUsize::new(0),
            glicko_predicted: AtomicUsize::new(0),
        }
    }
}

type MapType = HashMap<String, GlickoRating>;

pub struct RatingDB {
    db: Mutex<MapType>,
    stats: StatsDB,
}

impl RatingDB {
    pub fn new() -> RatingDB {
        RatingDB {
            db: Mutex::new(MapType::new()),
            stats: StatsDB::new(),
        }
    }

    pub fn player_count(&self) -> usize {
        self.db.lock().unwrap().len()
    }

    pub fn update(&mut self, update: ResultUpdate) {
        let result = update.result.unwrap();
        let res_time = update.datetime;

        let mut db = self.db.lock().unwrap();

        let white_entry = match db.get(&update.white) {
            Some(&entry) => entry,
            None => GlickoRating::default(),
        };
        let black_entry = match db.get(&update.black) {
            Some(&entry) => entry,
            None => GlickoRating::default(),
        };

        let new_white =
            white_entry.update_with_result(Color::White, &result, &res_time, black_entry);
        let new_black =
            black_entry.update_with_result(Color::Black, &result, &res_time, white_entry);

        db.insert(update.white, new_white);
        db.insert(update.black, new_black);
    }
}
