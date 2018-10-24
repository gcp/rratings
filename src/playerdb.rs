use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
use pgn_reader::{Color, Outcome};

use super::ResultUpdate;
use glicko::GlickoRating;

#[derive(Clone, Debug)]
pub struct Player {
    pub g1rating: GlickoRating,
    pub mtime: DateTime<Utc>,
}

impl Player {
    pub fn new(mtime: &DateTime<Utc>) -> Player {
        Player {
            g1rating: GlickoRating::new(),
            mtime: *mtime,
        }
    }

    pub fn update_with_result(
        &mut self,
        color: Color,
        result: &Outcome,
        result_time: &DateTime<Utc>,
        opponent: &Player,
    ) {
        let old_time = self.mtime;

        let score = match result {
            Outcome::Draw => 0.5,
            Outcome::Decisive { winner } => {
                if *winner == color {
                    1.0
                } else {
                    0.0
                }
            }
        };

        self.g1rating
            .update_with_result(score, &old_time, result_time, opponent);
        self.mtime = *result_time;
    }
}

pub struct StatsDB {
    glicko_guess: u64,
    glicko_predicted: u64,
    glicko_mse_accum: f64,
    glicko_mse_total: f64,
}

impl StatsDB {
    pub fn new() -> StatsDB {
        StatsDB {
            glicko_guess: 0,
            glicko_predicted: 0,
            glicko_mse_accum: 0.0,
            glicko_mse_total: 0.0,
        }
    }
}

type MapType = HashMap<String, Player>;

pub struct RatingDB {
    db: Mutex<MapType>,
    stats: Mutex<StatsDB>,
}

impl RatingDB {
    pub fn new() -> RatingDB {
        RatingDB {
            db: Mutex::new(MapType::new()),
            stats: Mutex::new(StatsDB::new()),
        }
    }

    pub fn player_count(&self) -> usize {
        self.db.lock().unwrap().len()
    }

    pub fn update(&mut self, update: ResultUpdate) {
        let result = update.result.unwrap();
        let res_time = update.datetime;

        let mut db = self.db.lock().unwrap();

        let mut white_entry = match db.get(&update.white) {
            Some(entry) => entry.clone(),
            None => Player::new(&res_time),
        };
        let mut black_entry = match db.get(&update.black) {
            Some(entry) => entry.clone(),
            None => Player::new(&res_time),
        };

        white_entry.update_with_result(Color::White, &result, &res_time, &black_entry);
        black_entry.update_with_result(Color::Black, &result, &res_time, &white_entry);

        db.insert(update.white, white_entry);
        db.insert(update.black, black_entry);
    }
}
