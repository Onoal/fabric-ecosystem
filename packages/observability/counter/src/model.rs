#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CounterIncrementResult {
    Updated { value: u64 },
    Overflow { current: u64, attempted: u64 },
}

impl CounterIncrementResult {
    pub fn value(&self) -> Option<u64> {
        match self {
            Self::Updated { value } => Some(*value),
            Self::Overflow { .. } => None,
        }
    }
}
