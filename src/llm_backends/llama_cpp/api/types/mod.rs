//! Types used in OpenAI API requests and responses.
//! These types are created from component schemas in the [OpenAPI spec](https://github.com/openai/openai-openapi)
mod impls;
use super::error::LlamaApiError;
use crate::llm_backends::llama_cpp::api::config::{Config, LlamaConfig};
use derive_builder::{Builder, UninitializedFieldError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl From<UninitializedFieldError> for LlamaApiError {
    fn from(value: UninitializedFieldError) -> Self {
        LlamaApiError::InvalidArgument(value.to_string())
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
/// Client is a container for config, backoff and http_client
/// used to make API calls.
pub struct Client<C: Config> {
    http_client: reqwest::Client,
    config: C,
    backoff: backoff::ExponentialBackoff,
}

impl Default for Client<LlamaConfig> {
    fn default() -> Self {
        Self::new()
    }
}
impl Client<LlamaConfig> {
    /// Client with default [LlamaConfig]
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            config: LlamaConfig::default(),
            backoff: Default::default(),
        }
    }
}

#[derive(Clone, Serialize, Default, Debug, Builder, Deserialize, PartialEq)]
#[builder(name = "LlamaCompletionsRequestArgs")]
#[builder(pattern = "mutable")]
#[builder(setter(into, strip_option), default)]
#[builder(derive(Debug))]
#[builder(build_fn(error = "LlamaApiError"))]
pub struct LlamaCompletionsRequest {
    pub prompt: Vec<u32>,

    #[serde(skip)]
    pub prompt_string: Option<String>,

    /// A formatted "Grammar" as a string.
    /// See: https://github.com/richardanaya/gbnf/blob/main/gbnf/src/lib.rs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,

    /// Re-use previously cached prompt from the last request if possible. This may prevent re-caching the prompt from scratch. Default: false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_prompt: Option<bool>,

    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on their existing frequency in the text so far, decreasing the model's likelihood to repeat the same line verbatim.
    ///
    /// [See more information about frequency and presence penalties.](https://platform.openai.com/docs/api-reference/parameter-details)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>, // min: -2.0, max: 2.0, default: 0

    /// Modify the likelihood of specified tokens appearing in the completion.
    ///
    /// Accepts a json object that maps tokens (specified by their token ID in the tokenizer) to an associated bias value from -100 to 100.
    /// Mathematically, the bias is added to the logits generated by the model prior to sampling.
    /// The exact effect will vary per model, but values between -1 and 1 should decrease or increase likelihood of selection;
    /// values like -100 or 100 should result in a ban or exclusive selection of the relevant token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<Vec<Vec<serde_json::Value>>>, // default: null

    /// The maximum number of [tokens](https://platform.openai.com/tokenizer) to generate in the chat completion.
    ///
    /// The total length of input tokens and generated tokens is limited by the model's context length. [Example Python code](https://github.com/openai/openai-cookbook/blob/main/examples/How_to_count_tokens_with_tiktoken.ipynb) for counting tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_predict: Option<u32>,

    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on whether they appear in the text so far, increasing the model's likelihood to talk about new topics.
    ///
    /// [See more information about frequency and presence penalties.](https://platform.openai.com/docs/api-reference/parameter-details)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>, // min: -2.0, max: 2.0, default 0

    /// stop: Specify a JSON array of stopping strings.
    /// These words will not be included in the completion,
    /// so make sure to add them to the prompt for the next iteration (default: []).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// If set, partial message deltas will be sent, like in ChatGPT.
    /// Tokens will be sent as data-only [server-sent events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#Event_stream_format)
    /// as they become available, with the stream terminated by a `data: [DONE]` message. [Example Python code](https://cookbook.openai.com/examples/how_to_stream_completions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random,
    /// while lower values like 0.2 will make it more focused and deterministic.
    ///
    /// We generally recommend altering this or `top_p` but not both.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>, // min: 0, max: 2, default: 1,

    /// An alternative to sampling with temperature, called nucleus sampling,
    /// where the model considers the results of the tokens with top_p probability mass.
    /// So 0.1 means only the tokens comprising the top 10% probability mass are considered.
    ///
    ///  We generally recommend altering this or `temperature` but not both.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>, // min: 0, max: 1, default: 1
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct LlamaResponse {
    pub content: String,
    pub model: String,
    // pub prompt: String, // Need to think how to handle tokens vs. text
    pub generation_settings: LlamaGenerationSettings,
    pub stop: bool,
    pub stopped_eos: bool,
    pub stopped_limit: bool,
    pub stopped_word: bool,
    pub stopping_word: String,
    pub timings: HashMap<String, f32>,
    pub tokens_cached: u16,
    pub tokens_evaluated: u16,
    pub truncated: bool,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct LlamaGenerationSettings {
    pub n_ctx: u16,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
    pub temperature: f32,
    pub top_p: f32,
    pub n_predict: i16,
    pub logit_bias: Vec<Vec<serde_json::Value>>,
    pub grammar: String,
    pub stop: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum Stop {
    String(String),           // nullable: true
    StringArray(Vec<String>), // minItems: 1; maxItems: 4
}

#[derive(Debug, Serialize, Default, Clone, Builder, PartialEq)]
#[builder(name = "LlamaCreateEmbeddingRequestArgs")]
#[builder(pattern = "mutable")]
#[builder(setter(into, strip_option), default)]
#[builder(derive(Debug))]
#[builder(build_fn(error = "LlamaApiError"))]
pub struct LlamaCreateEmbeddingRequest {
    pub content: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct LlamaCreateEmbeddingResponse {
    pub embedding: Vec<f32>,
}
