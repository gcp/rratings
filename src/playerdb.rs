use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::ResultUpdate;
use glicko::GlickoRating;

pub struct Player {
    name: String,
    rating: GlickoRating,
}

impl Player {
    pub fn new(name: &str) -> Player {
        Player {
            name: name.to_string(),
            rating: GlickoRating::new(),
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

    pub fn update(&mut self, update: &ResultUpdate) {}
}
