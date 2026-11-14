/// Cycles through a fixed set of values with wrap-around.
#[derive(Debug, Clone, Copy)]
pub struct Cycle<'a, T> {
    values: &'a [T],
}

impl<'a, T: PartialEq> Cycle<'a, T> {
    pub const fn new(values: &'a [T]) -> Self {
        Self { values }
    }

    pub fn step(&self, current: &T, forward: bool) -> Option<&'a T> {
        if self.values.is_empty() {
            return None;
        }

        let index = self
            .values
            .iter()
            .position(|value| value == current)
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % self.values.len()
        } else {
            (index + self.values.len() - 1) % self.values.len()
        };
        self.values.get(next)
    }
}
