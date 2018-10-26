use std::cmp;
use std::f32::consts;
use std::fmt;

use chrono::{DateTime, TimeZone, Utc};
use pgn_reader::{Color, Outcome};
use playerdb::Player;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlickoRating {
    pub r: f32,
    pub rd: f32,
}

impl fmt::Display for GlickoRating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}±{}", self.r, self.rd)
    }
}

impl GlickoRating {
    // 350 = sqrt(50^2 + c^2 * 1825)
    // 350^2 = 50^2 + c^2 * 1825
    // 350^2 - 50^2 = c^2 * 1825
    // 350^2 - 50^2 / 1825 = c^2
    // c = sqrt((350^2 - 50^2) / 1825)
    // c = ~8.11
    const DAYS_UNTIL_UNRATED: f32 = 5.0 * 365.0;
    const C_2: f32 = ((350.0 * 350.0) - (50.0 * 50.0)) / GlickoRating::DAYS_UNTIL_UNRATED;
    // ln 10 / 400
    const Q: f32 = 0.0057565;

    pub fn new() -> GlickoRating {
        GlickoRating {
            r: 1500.0,
            rd: 350.0,
        }
    }

    fn calc_e(rd: f32, r1: f32, r2: f32) -> f32 {
        let a = -GlickoRating::calc_g(rd) * (r2 - r1) / 400.0;
        let p = 10.0f32.powf(a);
        1.0 / (1.0 + p)
    }

    fn calc_expect(rd1: f32, rd2: f32, r1: f32, r2: f32) -> f32 {
        let rd = (rd1.powf(2.0) + rd2.powf(2.0)).sqrt();
        GlickoRating::calc_e(rd, r1, r2)
    }

    fn calc_g(rd: f32) -> f32 {
        let rdsq = rd.powf(2.0);
        let nom = 1.0 + ((3.0 * GlickoRating::Q.powf(2.0) * rdsq) / (consts::PI.powf(2.0)));
        1.0 / nom.sqrt()
    }

    fn calc_new_rd(&self, days: f32) -> f32 {
        let new_rd = (self.rd.powf(2.0) + (days * GlickoRating::C_2)).sqrt();
        new_rd.min(350.0)
    }

    fn calc_days(old: &DateTime<Utc>, now: &DateTime<Utc>) -> f32 {
        let duration = *now - *old;
        // days returns an integer
        let seconds = duration.num_seconds() as f64;
        let days = seconds / (24.0 * 60.0 * 60.0);
        days as f32
    }

    pub fn expect(
        self,
        old_time: &DateTime<Utc>,
        result_time: &DateTime<Utc>,
        opponent: &Player,
    ) -> f32 {
        let days_me = GlickoRating::calc_days(old_time, result_time);
        let days_him = GlickoRating::calc_days(&opponent.mtime, result_time);

        let pre_rd_me = self.calc_new_rd(days_me);
        let pre_rd_his = opponent.g1rating.calc_new_rd(days_him);

        GlickoRating::calc_expect(pre_rd_his, pre_rd_me, opponent.g1rating.r, self.r)
    }

    pub fn update_with_result(
        &mut self,
        score: f32,
        old_time: &DateTime<Utc>,
        result_time: &DateTime<Utc>,
        opponent: &Player,
    ) {
        let days_me = GlickoRating::calc_days(old_time, result_time);
        let days_him = GlickoRating::calc_days(&opponent.mtime, result_time);

        let pre_rd_me = self.calc_new_rd(days_me);
        let pre_rd_his = opponent.g1rating.calc_new_rd(days_him);

        let e = GlickoRating::calc_e(pre_rd_his, opponent.g1rating.r, self.r);
        let g = GlickoRating::calc_g(pre_rd_his);

        let d_2 = 1.0 / (GlickoRating::Q.powf(2.0) * g.powf(2.0) * e * (1.0 - e));

        let q_mul = GlickoRating::Q / ((1.0 / pre_rd_me.powf(2.0)) + (1.0 / d_2));

        let new_rating = self.r + q_mul * g * (score - e);
        let new_rd_sqr = 1.0 / ((1.0 / pre_rd_me.powf(2.0)) + (1.0 / d_2));
        let new_rd = new_rd_sqr.sqrt();

        self.r = new_rating;
        self.rd = new_rd.max(30.0);
    }
}

impl Default for GlickoRating {
    fn default() -> GlickoRating {
        GlickoRating::new()
    }
}
