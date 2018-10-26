use std::cmp;
use std::f32::consts;
use std::fmt;

use chrono::{DateTime, TimeZone, Utc};
use pgn_reader::{Color, Outcome};
use playerdb::Player;
use roots::{find_root_regula_falsi, SimpleConvergency};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ligcko2Rating {
    pub mu: f32,
    pub phi: f32,
    pub sigma: f32,
}

impl fmt::Display for Ligcko2Rating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1},{:.1},{:.4}", self.r(), self.rd(), self.sigma)
    }
}

impl Ligcko2Rating {
    const TAU: f32 = 0.75;
    const QF: f32 = 173.7178;
    const VOLATILITY: f32 = 0.06;
    // Chosen so a typical player's RD goes from 60 -> 110 in 1 year
    const RATING_PERIOD_DAYS: f32 = 4.665;
    const MAX_PHI: f32 = 350.0 / Ligcko2Rating::QF;
    const MIN_PHI: f32 = 60.0 / Ligcko2Rating::QF;
    const MAX_VOLATILITY: f32 = 0.1;

    pub fn new() -> Ligcko2Rating {
        Ligcko2Rating {
            mu: 0.0,
            phi: 350.0 / Ligcko2Rating::QF,
            sigma: Ligcko2Rating::VOLATILITY,
        }
    }

    pub fn r(self) -> f32 {
        1500.0 + self.mu * Ligcko2Rating::QF
    }

    pub fn rd(self) -> f32 {
        self.phi * Ligcko2Rating::QF
    }

    fn calc_e(phi: f32, mu1: f32, mu2: f32) -> f32 {
        let a = -Ligcko2Rating::calc_g(phi) * (mu2 - mu1);
        let p = a.exp();
        1.0 / (1.0 + p)
    }

    fn calc_expect(phi1: f32, phi2: f32, mu1: f32, mu2: f32) -> f32 {
        let phi = (phi1.powf(2.0) + phi2.powf(2.0)).sqrt();
        Ligcko2Rating::calc_e(phi, mu1, mu2)
    }

    fn calc_g(phi: f32) -> f32 {
        let nom = 1.0 + ((3.0 * phi.powf(2.0)) / (consts::PI.powf(2.0)));
        1.0 / nom.sqrt()
    }

    fn calc_days(old: &DateTime<Utc>, now: &DateTime<Utc>) -> f32 {
        let duration = *now - *old;
        // days returns an integer
        let seconds = duration.num_seconds() as f64;
        let days = seconds / (24.0 * 60.0 * 60.0);
        days as f32
    }

    fn calc_new_phi(&self, days: f32) -> f32 {
        let new_phi = (self.phi.powf(2.0)
            + ((days / Ligcko2Rating::RATING_PERIOD_DAYS) * self.sigma.powf(2.0))).sqrt();
        new_phi.min(Ligcko2Rating::MAX_PHI)
    }

    pub fn expect(
        self,
        old_time: &DateTime<Utc>,
        result_time: &DateTime<Utc>,
        opponent: &Player,
    ) -> f32 {
        let days_me = Ligcko2Rating::calc_days(old_time, result_time);
        let days_him = Ligcko2Rating::calc_days(&opponent.mtime, result_time);

        let pre_phi_me = self.calc_new_phi(days_me);
        let pre_phi_his = opponent.l2rating.calc_new_phi(days_him);

        Ligcko2Rating::calc_expect(pre_phi_his, pre_phi_me, opponent.l2rating.mu, self.mu)
    }

    pub fn update_with_result(
        &mut self,
        score: f32,
        old_time: &DateTime<Utc>,
        result_time: &DateTime<Utc>,
        opponent: &Player,
    ) {
        let days_me = Ligcko2Rating::calc_days(old_time, result_time);
        let days_him = Ligcko2Rating::calc_days(&opponent.mtime, result_time);

        // Not used in Glicko-2?
        // This means we overstate the reliability of the opponent's rating
        // let pre_phi_his = opponent.l2rating.calc_new_phi(days_him);
        // This is calculated with the new sigma
        // let pre_phi_me = self.calc_new_phi(days_me);

        let e = Ligcko2Rating::calc_e(opponent.l2rating.phi, opponent.l2rating.mu, self.mu);
        let g = Ligcko2Rating::calc_g(opponent.l2rating.phi);

        let v = 1.0 / (g.powf(2.0) * e * (1.0 - e));
        let delta = v * g * (score - e);

        let orig_phi = self.phi;
        let a = (self.sigma.powf(2.0)).ln();

        let f = |x: f32| {
            (x.exp() * (delta.powf(2.0) - orig_phi.powf(2.0) - v - x.exp())
                / (2.0 * (orig_phi.powf(2.0) + v + x.exp()).powf(2.0)))
                - ((x - a) / Ligcko2Rating::TAU.powf(2.0))
        };

        let b = if delta.powf(2.0) > orig_phi.powf(2.0) + v {
            (delta.powf(2.0) - orig_phi.powf(2.0) - v).ln()
        } else {
            let mut k = 1.0;
            while f(a - k * Ligcko2Rating::TAU) < 0.0 {
                k += 1.0;
            }
            a - k * Ligcko2Rating::TAU
        };

        let mut convergency = SimpleConvergency {
            eps: 0.00001,
            max_iter: 30,
        };
        let root = find_root_regula_falsi(a, b, &f, &mut convergency).unwrap();

        let sigma = (root / 2.0).exp();

        let phi_star = (orig_phi.powf(2.0)
            + ((days_me / Ligcko2Rating::RATING_PERIOD_DAYS) * sigma.powf(2.0))).sqrt();
        let phi = 1.0 / ((1.0 / phi_star.powf(2.0)) + (1.0 / v)).sqrt();

        let mu = self.mu + phi.powf(2.0) * g * (score - e);

        self.mu = mu;
        self.phi = phi.max(Ligcko2Rating::MIN_PHI);
        self.sigma = sigma.min(Ligcko2Rating::MAX_VOLATILITY);
    }
}

impl Default for Ligcko2Rating {
    fn default() -> Ligcko2Rating {
        Ligcko2Rating::new()
    }
}
