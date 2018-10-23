use std::fmt;

use chrono::{DateTime, TimeZone, Utc};
use pgn_reader::{Color, Outcome};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlickoRating {
    r: f32,
    rd: f32,
}

impl fmt::Display for GlickoRating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}±{}", self.r, self.rd)
    }
}

impl GlickoRating {
    pub fn new() -> GlickoRating {
        GlickoRating {
            r: 1500.0,
            rd: 350.0,
        }
    }

    pub fn update_with_result(
        &self,
        color: Color,
        result: &Outcome,
        result_time: &DateTime<Utc>,
        opponent: GlickoRating,
    ) -> GlickoRating {
        self.clone()
    }
}

impl Default for GlickoRating {
    fn default() -> GlickoRating {
        GlickoRating::new()
    }
}
