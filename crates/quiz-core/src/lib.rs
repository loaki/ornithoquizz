//! Question generation, answer checking and spaced repetition. No I/O, no UI: fully testable.

pub mod leitner;
pub mod question;
pub mod rng;
pub mod text;

pub use question::{generate, Difficulty, Question};
