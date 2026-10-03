//! RI-08 selected safe-trait fixture, unchanged between capture and execution.
pub trait Accumulator {
    type Error;
    fn advance(&mut self, value: i64) -> Result<i64, Self::Error>;
}
