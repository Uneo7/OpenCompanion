use crate::ui::UiStep;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub title: String,
    pub writes: Vec<String>,
    pub warnings: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }

    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![self.title.clone()];
        out.extend(self.writes.iter().map(|w| format!("  - {w}")));
        out.extend(self.warnings.iter().map(|w| format!("  ! {w}")));
        out
    }
}

pub trait Host {
    fn log(&mut self, line: &str);

    /// Asked before the first EEPROM write; `false` cancels (nothing written).
    fn confirm(&mut self, plan: &Plan) -> bool;

    fn step(&mut self, step: UiStep) {
        let _ = step;
    }

    /// Called around every EEPROM-writing phase, critical window included.
    fn keep_awake(&mut self, on: bool);
}
