//! Calendar date type and month math.
//!
//! Zero-dependency date helpers: weekday calculation, leap year handling,
//! day/month arithmetic with clamping.

/// Simple date value (no timezone, no time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl CalDate {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Day of week: 0 = Monday … 6 = Sunday.
    pub fn weekday(&self) -> u32 {
        let t = [0i32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let mut y = self.year;
        if self.month < 3 {
            y -= 1;
        }
        let dow = (y + y / 4 - y / 100 + y / 400 + t[(self.month - 1) as usize] + self.day as i32)
            % 7;
        ((dow + 6) % 7) as u32
    }

    pub fn days_in_month(&self) -> u32 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.year % 4 == 0 && (self.year % 100 != 0 || self.year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    pub fn next_day(self) -> Self {
        if self.day < self.days_in_month() {
            Self { day: self.day + 1, ..self }
        } else if self.month < 12 {
            Self { month: self.month + 1, day: 1, ..self }
        } else {
            Self { year: self.year + 1, month: 1, day: 1 }
        }
    }

    pub fn prev_day(self) -> Self {
        if self.day > 1 {
            Self { day: self.day - 1, ..self }
        } else if self.month > 1 {
            let pm = Self { month: self.month - 1, day: 1, ..self };
            Self { day: pm.days_in_month(), ..pm }
        } else {
            Self { year: self.year - 1, month: 12, day: 31 }
        }
    }

    pub fn add_days(self, n: i32) -> Self {
        let mut d = self;
        if n >= 0 {
            for _ in 0..n {
                d = d.next_day();
            }
        } else {
            for _ in 0..(-n) {
                d = d.prev_day();
            }
        }
        d
    }

    pub fn next_month(self) -> Self {
        let (y, m) = if self.month == 12 {
            (self.year + 1, 1)
        } else {
            (self.year, self.month + 1)
        };
        let dim = CalDate::new(y, m, 1).days_in_month();
        CalDate::new(y, m, self.day.min(dim))
    }

    pub fn prev_month(self) -> Self {
        let (y, m) = if self.month == 1 {
            (self.year - 1, 12)
        } else {
            (self.year, self.month - 1)
        };
        let dim = CalDate::new(y, m, 1).days_in_month();
        CalDate::new(y, m, self.day.min(dim))
    }

    /// Full month name: "January", "February", etc.
    pub fn month_name(&self) -> &'static str {
        [
            "January", "February", "March", "April", "May", "June",
            "July", "August", "September", "October", "November", "December",
        ]
        .get((self.month as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("???")
    }

    /// Short month name: "Jan", "Feb", etc.
    pub fn month_name_short(&self) -> &'static str {
        [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .get((self.month as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("???")
    }

    pub fn format(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl std::fmt::Display for CalDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_in_month_leap() {
        assert_eq!(CalDate::new(2024, 2, 1).days_in_month(), 29);
        assert_eq!(CalDate::new(2026, 2, 1).days_in_month(), 28);
        assert_eq!(CalDate::new(1900, 2, 1).days_in_month(), 28);
        assert_eq!(CalDate::new(2000, 2, 1).days_in_month(), 29);
    }

    #[test]
    fn next_day_cross_month() {
        assert_eq!(CalDate::new(2026, 3, 31).next_day(), CalDate::new(2026, 4, 1));
    }

    #[test]
    fn next_day_cross_year() {
        assert_eq!(CalDate::new(2026, 12, 31).next_day(), CalDate::new(2027, 1, 1));
    }

    #[test]
    fn prev_day_cross_month() {
        assert_eq!(CalDate::new(2026, 4, 1).prev_day(), CalDate::new(2026, 3, 31));
    }

    #[test]
    fn next_month_clamps() {
        assert_eq!(CalDate::new(2026, 3, 31).next_month(), CalDate::new(2026, 4, 30));
    }

    #[test]
    fn prev_month_clamps() {
        assert_eq!(CalDate::new(2026, 3, 31).prev_month(), CalDate::new(2026, 2, 28));
    }

    #[test]
    fn weekday_known_date() {
        // 2026-04-01 is a Wednesday = 2
        assert_eq!(CalDate::new(2026, 4, 1).weekday(), 2);
    }

    #[test]
    fn month_names() {
        assert_eq!(CalDate::new(2026, 1, 1).month_name(), "January");
        assert_eq!(CalDate::new(2026, 12, 1).month_name(), "December");
        assert_eq!(CalDate::new(2026, 6, 1).month_name_short(), "Jun");
    }

    #[test]
    fn display_format() {
        assert_eq!(CalDate::new(2026, 4, 1).to_string(), "2026-04-01");
        assert_eq!(CalDate::new(2026, 4, 1).format(), "2026-04-01");
    }

    #[test]
    fn add_days_forward_and_back() {
        let d = CalDate::new(2026, 3, 15);
        assert_eq!(d.add_days(20), CalDate::new(2026, 4, 4));
        assert_eq!(d.add_days(-15), CalDate::new(2026, 2, 28));
    }
}
