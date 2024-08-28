pub mod boolean;
pub mod exact_string;
pub mod integer;
pub mod sentences;
pub mod text;
pub mod words;

use anyhow::Result;
pub use boolean::BooleanPrimitive;
pub use exact_string::ExactStringPrimitive;
pub use integer::IntegerPrimitive;
use llm_utils::grammar::Grammar;
pub use sentences::SentencesPrimitive;
pub use text::TextPrimitive;
pub use words::WordsPrimitive;

pub trait PrimitiveTrait: Default {
    type PrimitiveResult: std::str::FromStr + std::fmt::Display;

    fn clear_primitive(&mut self);

    fn type_description(&self, result_can_be_none: bool) -> &str;

    fn solution_description(&self, result_can_be_none: bool) -> String;

    fn stop_word_result_is_none(&self, result_can_be_none: bool) -> Option<String>;

    fn grammar(&self) -> Grammar;

    fn parse_to_primitive(&self, content: &str) -> Result<Self::PrimitiveResult>;
}
