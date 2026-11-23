use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spin {
    None,
    Mini,
    Full,
}

// things that affect score:
// b2b
// combo
// spin type
// line count
// level (not counted here)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub b2b: bool,
    pub combo: Option<u32>,
    pub spin: Spin,
    pub lines: u32,
    pub pc: bool,
}

impl Event {
    #[must_use]
    pub const fn score(&self) -> u32 {
        debug_assert!(self.lines <= 4);
        debug_assert!(!self.pc || self.lines > 0);

        let plain = match self.lines {
            0 => 0,
            1 => 100,
            2 => 300,
            3 => 500,
            _ => 800,
        };
        let base = match self.spin {
            Spin::None => plain,
            Spin::Mini => plain + 100,
            Spin::Full => 400 * (self.lines + 1),
        };

        // A zero-line event has no B2B, combo, or perfect clear term.
        if self.lines == 0 {
            return base;
        }

        let difficult = !matches!(self.spin, Spin::None) || self.lines == 4;
        let mut score = if self.b2b && difficult {
            base + base / 2
        } else {
            base
        };
        if let Some(combo) = self.combo {
            score += 50 * combo;
        }
        if self.pc {
            score += 3000;
        }
        score
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let spin = match self.spin {
            Spin::None => "",
            Spin::Mini => "TSM",
            Spin::Full => "TS",
        };

        let base = match self.spin {
            Spin::None => match self.lines {
                0 => "0",
                1 => "1",
                2 => "2",
                3 => "3",
                _ => "Quad",
            },
            _ => match self.lines {
                0 => "0",
                1 => "S",
                2 => "D",
                3 => "T",
                _ => "Q",
            },
        };
        write!(
            f,
            "{}{spin}{base}{}",
            if self.b2b { "B2B " } else { "" },
            if let Some(combo) = self.combo
                && let 1.. = combo
            {
                format!("+{combo}")
            } else {
                String::new()
            }
        )
    }
}
