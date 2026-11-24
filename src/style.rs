//! Tiny ANSI styling helper (no dependency, Windows-10+ aware).

#[derive(Clone, Copy)]
pub struct Style {
    pub enabled: bool,
}

impl Style {
    pub fn new(enabled: bool) -> Self {
        Style { enabled }
    }

    fn wrap(&self, code: &str, s: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    pub fn bold(&self, s: &str) -> String {
        self.wrap("1", s)
    }
    pub fn dim(&self, s: &str) -> String {
        self.wrap("2", s)
    }
    pub fn red(&self, s: &str) -> String {
        self.wrap("31", s)
    }
    pub fn green(&self, s: &str) -> String {
        self.wrap("32", s)
    }
    pub fn yellow(&self, s: &str) -> String {
        self.wrap("33", s)
    }
    pub fn blue(&self, s: &str) -> String {
        self.wrap("34", s)
    }
    pub fn magenta(&self, s: &str) -> String {
        self.wrap("35", s)
    }
    pub fn cyan(&self, s: &str) -> String {
        self.wrap("36", s)
    }

    /// Colour a status code by its class, padded so log columns line up.
    pub fn status(&self, code: u16, text: &str) -> String {
        let t = format!("{code} {text:<12}");
        match code {
            200..=299 => self.green(&t),
            300..=399 => self.cyan(&t),
            400..=499 => self.yellow(&t),
            _ => self.red(&t),
        }
    }

    pub fn method(&self, m: &str) -> String {
        let padded = format!("{m:<7}");
        match m {
            "GET" => self.green(&padded),
            "HEAD" => self.cyan(&padded),
            "OPTIONS" => self.blue(&padded),
            _ => self.magenta(&padded),
        }
    }
}
