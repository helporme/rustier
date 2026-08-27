#[derive(Clone)]
pub enum Phase {
    Waiting,
    Running { step: i32 },
    Complete,
}

impl Phase {
    pub fn waiting() -> Self {
        Self::Waiting
    }

    pub fn running(step: i32) -> Self {
        Self::Running { step }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    pub fn finish(&mut self) {
        *self = Self::Complete;
    }

    pub fn step(&self) -> i32 {
        match self {
            Self::Running { step } => *step,
            Self::Waiting | Self::Complete => -1,
        }
    }

    pub fn into_step(self) -> i32 {
        self.step()
    }

    pub fn same_phase(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    pub fn clone_of(other: &Self) -> Self {
        other.clone()
    }
}
