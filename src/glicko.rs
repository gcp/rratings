use std::fmt;

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
}
