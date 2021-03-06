use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
use pgn_reader::{Color, Outcome};

use super::ResultUpdate;
use glicko::GlickoRating;
use glicko2::Glicko2Rating;
use ligcko2::Ligcko2Rating;

#[derive(Clone, Debug)]
pub struct Player {
    pub g1rating: GlickoRating,
    pub g2rating: Glicko2Rating,
    pub l2rating: Ligcko2Rating,
    pub mtime: DateTime<Utc>,
}

impl Player {
    pub fn new(mtime: &DateTime<Utc>) -> Player {
        Player {
            g1rating: GlickoRating::new(),
            g2rating: Glicko2Rating::new(),
            l2rating: Ligcko2Rating::new(),
            mtime: *mtime,
        }
    }

    pub fn update_with_result(
        &mut self,
        color: Color,
        result: &Outcome,
        result_time: &DateTime<Utc>,
        opponent: &Player,
        stats: Option<&Mutex<StatsDB>>,
        update: Option<&ResultUpdate>,
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

        // Record predictions
        if let Some(stats) = stats {
            let mut stats = stats.lock().unwrap();

            // Glicko-1 updates
            // Ternary expected, for compat with lichess ratings
            let expected_score = if self.g1rating.r > opponent.g1rating.r {
                1.0f32
            } else if self.g1rating.r < opponent.g1rating.r {
                0.0f32
            } else {
                0.5f32
            };
            stats.glicko_guess += 1;
            if (score - expected_score).abs() < 0.5f32 {
                stats.glicko_predicted += 1
            }

            // Smooth expected from Glicko formula
            let expected_score = self.g1rating.expect(&old_time, result_time, opponent);
            stats.glicko_mse_total += 1.0;
            stats.glicko_mse_accum += (score - expected_score).powf(2.0) as f64;

            // Glicko-2 updates
            // Ternary expected, for compat with lichess ratings
            let expected_score = if self.g2rating.mu > opponent.g2rating.mu {
                1.0f32
            } else if self.g2rating.mu < opponent.g2rating.mu {
                0.0f32
            } else {
                0.5f32
            };
            stats.glicko2_guess += 1;
            if (score - expected_score).abs() < 0.5f32 {
                stats.glicko2_predicted += 1
            }

            // Smooth expected from Glicko formula
            let expected_score = self.g2rating.expect(opponent);
            stats.glicko2_mse_total += 1.0;
            stats.glicko2_mse_accum += (score - expected_score).powf(2.0) as f64;

            // Ligcko-2 updates
            // Ternary expected, for compat with lichess ratings
            let expected_score = if self.l2rating.mu > opponent.l2rating.mu {
                1.0f32
            } else if self.l2rating.mu < opponent.l2rating.mu {
                0.0f32
            } else {
                0.5f32
            };
            stats.ligcko2_guess += 1;
            if (score - expected_score).abs() < 0.5f32 {
                stats.ligcko2_predicted += 1
            }

            // Smooth expected from Glicko formula
            let expected_score = self.l2rating.expect(&old_time, result_time, opponent);
            stats.ligcko2_mse_total += 1.0;
            stats.ligcko2_mse_accum += (score - expected_score).powf(2.0) as f64;

            // Lichess' own rating updates
            if let Some(update) = update {
                assert!(color == Color::White);
                let expected_score = if update.white_rating > update.black_rating {
                    1.0f32
                } else if update.white_rating < update.black_rating {
                    0.0f32
                } else {
                    0.5f32
                };
                stats.lichess_guess += 1;
                if (score - expected_score).abs() < 0.5f32 {
                    stats.lichess_predicted += 1
                }
            }
        }

        // Update ratings
        self.g1rating
            .update_with_result(score, &old_time, result_time, opponent);
        self.g2rating.update_with_result(score, opponent);
        self.l2rating
            .update_with_result(score, &old_time, result_time, opponent);
        self.mtime = *result_time;
    }
}

pub struct StatsDB {
    pub glicko_guess: u64,
    pub glicko_predicted: u64,
    pub glicko_mse_accum: f64,
    pub glicko_mse_total: f64,
    pub glicko2_guess: u64,
    pub glicko2_predicted: u64,
    pub glicko2_mse_accum: f64,
    pub glicko2_mse_total: f64,
    pub ligcko2_guess: u64,
    pub ligcko2_predicted: u64,
    pub ligcko2_mse_accum: f64,
    pub ligcko2_mse_total: f64,
    pub lichess_guess: u64,
    pub lichess_predicted: u64,
}

impl StatsDB {
    pub fn new() -> StatsDB {
        StatsDB {
            glicko_guess: 0,
            glicko_predicted: 0,
            glicko_mse_accum: 0.0,
            glicko_mse_total: 0.0,
            glicko2_guess: 0,
            glicko2_predicted: 0,
            glicko2_mse_accum: 0.0,
            glicko2_mse_total: 0.0,
            ligcko2_guess: 0,
            ligcko2_predicted: 0,
            ligcko2_mse_accum: 0.0,
            ligcko2_mse_total: 0.0,
            lichess_guess: 0,
            lichess_predicted: 0,
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

        // Pass and update prediction stats for white only
        white_entry.update_with_result(
            Color::White,
            &result,
            &res_time,
            &black_entry,
            Some(&self.stats),
            Some(&update),
        );
        black_entry.update_with_result(Color::Black, &result, &res_time, &white_entry, None, None);

        db.insert(update.white, white_entry);
        db.insert(update.black, black_entry);
    }

    pub fn get_stats(&self) -> String {
        let stats = self.stats.lock().unwrap();

        let pred_rate = 100.0 * stats.glicko_predicted as f64 / stats.glicko_guess as f64;
        let mse = stats.glicko_mse_accum / stats.glicko_mse_total;

        let pred_rate_g2 = 100.0 * stats.glicko2_predicted as f64 / stats.glicko2_guess as f64;
        let mse_g2 = stats.glicko2_mse_accum / stats.glicko2_mse_total;

        let pred_rate_l2 = 100.0 * stats.ligcko2_predicted as f64 / stats.ligcko2_guess as f64;
        let mse_l2 = stats.ligcko2_mse_accum / stats.ligcko2_mse_total;

        let lichess_pred_rate = 100.0 * stats.lichess_predicted as f64 / stats.lichess_guess as f64;

        let mut out = format!("{:.3}% G1 p-rate, {:.4} G1 MSE", pred_rate, mse);
        out += &format!(", {:.3}% G2 p-rate, {:.4} G2 MSE", pred_rate_g2, mse_g2);
        out += &format!(", {:.3}% L2 p-rate, {:.4} L2 MSE", pred_rate_l2, mse_l2);
        out += &format!(", {:.3}% lichess p-rate ", lichess_pred_rate);

        out
    }

    pub fn stats_reset(&mut self) {
        self.stats = Mutex::new(StatsDB::new());
    }

    pub fn dump_report(self, report_filename: &str) {
        let mut db = self.db.lock().unwrap();
        let mut sort_db: Vec<_> = db.drain().collect();
        // Sort by lower confidence bound of Glicko-1 rating
        sort_db.sort_by_key(|x| -((x.1.g1rating.r as i32) - 2 * (x.1.g1rating.rd as i32)));

        let mut file = File::create(report_filename).unwrap();

        for (key, val) in sort_db.iter() {
            let player = format!(
                "{},{},{},{}\n",
                key, val.g1rating, val.g2rating, val.l2rating,
            );
            file.write_all(player.as_bytes()).unwrap();
        }
    }
}
