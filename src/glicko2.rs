use std::cmp;
use std::f32::consts;
use std::fmt;

use chrono::{DateTime, TimeZone, Utc};
use pgn_reader::{Color, Outcome};
use playerdb::Player;
use roots::{find_root_regula_falsi, SimpleConvergency};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glicko2Rating {
    pub mu: f32,
    pub phi: f32,
    pub sigma: f32,
}

impl fmt::Display for Glicko2Rating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1},{:.1},{:.4}", self.r(), self.rd(), self.sigma)
    }
}

impl Glicko2Rating {
    const TAU: f32 = 0.75;
    const QF: f32 = 173.7178;
    const VOLATILITY: f32 = 0.06;

    pub fn new() -> Glicko2Rating {
        Glicko2Rating {
            mu: 0.0,
            phi: 350.0 / Glicko2Rating::QF,
            sigma: Glicko2Rating::VOLATILITY,
        }
    }

    pub fn r(self) -> f32 {
        1500.0 + self.mu * Glicko2Rating::QF
    }

    pub fn rd(self) -> f32 {
        self.phi * Glicko2Rating::QF
    }

    fn calc_e(phi: f32, r1: f32, r2: f32) -> f32 {
        let a = -Glicko2Rating::calc_g(phi) * (r2 - r1);
        let p = a.exp();
        1.0 / (1.0 + p)
    }

    fn calc_expect(rd1: f32, rd2: f32, r1: f32, r2: f32) -> f32 {
        let rd = (rd1.powf(2.0) + rd2.powf(2.0)).sqrt();
        Glicko2Rating::calc_e(rd, r1, r2)
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

    pub fn expect(self, opponent: &Player) -> f32 {
        let pre_phi_me = (self.phi.powf(2.0) + self.sigma.powf(2.0)).sqrt();
        let pre_phi_his =
            (opponent.g2rating.phi.powf(2.0) + opponent.g2rating.sigma.powf(2.0)).sqrt();
        Glicko2Rating::calc_expect(pre_phi_his, pre_phi_me, opponent.g2rating.mu, self.mu)
    }

    pub fn update_with_result(&mut self, score: f32, opponent: &Player) {
        let e = Glicko2Rating::calc_e(opponent.g2rating.phi, opponent.g2rating.mu, self.mu);
        let g = Glicko2Rating::calc_g(opponent.g2rating.phi);

        let v = 1.0 / (g.powf(2.0) * e * (1.0 - e));
        let delta = v * g * (score - e);

        let orig_phi = self.phi;
        let a = (self.sigma.powf(2.0)).ln();

        let f = |x: f32| {
            (x.exp() * (delta.powf(2.0) - orig_phi.powf(2.0) - v - x.exp())
                / (2.0 * (orig_phi.powf(2.0) + v + x.exp()).powf(2.0)))
                - ((x - a) / Glicko2Rating::TAU.powf(2.0))
        };

        let b = if delta.powf(2.0) > orig_phi.powf(2.0) + v {
            (delta.powf(2.0) - orig_phi.powf(2.0) - v).ln()
        } else {
            let mut k = 1.0;
            while f(a - k * Glicko2Rating::TAU) < 0.0 {
                k += 1.0;
            }
            a - k * Glicko2Rating::TAU
        };

        let mut convergency = SimpleConvergency {
            eps: 0.00001,
            max_iter: 30,
        };
        let root = find_root_regula_falsi(a, b, &f, &mut convergency).unwrap();

        let sigma = (root / 2.0).exp();

        let pre_phi = (orig_phi.powf(2.0) + sigma.powf(2.0)).sqrt();
        let phi = 1.0 / ((1.0 / pre_phi.powf(2.0)) + (1.0 / v)).sqrt();

        let mu = self.mu + phi.powf(2.0) * g * (score - e);

        self.mu = mu;
        self.phi = phi;
        self.sigma = sigma;
    }
}

impl Default for Glicko2Rating {
    fn default() -> Glicko2Rating {
        Glicko2Rating::new()
    }
}
