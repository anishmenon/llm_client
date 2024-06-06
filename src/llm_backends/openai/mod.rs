use crate::{
    logging,
    response::parse_text_generation_response,
    LlmBackend,
    LlmClient,
    RequestConfig,
};
use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client as OpenAiClient,
};
use dotenv::dotenv;
use llm_utils::models::openai::OpenAiLlm;
use std::collections::HashMap;

pub struct OpenAiBackend {
    client: Option<OpenAiClient<OpenAIConfig>>,
    api_key: Option<String>,
    pub model: OpenAiLlm,
    pub logging_enabled: bool,
    tracing_guard: Option<tracing::subscriber::DefaultGuard>,
}

impl Default for OpenAiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiBackend {
    pub fn new() -> Self {
        let model = OpenAiLlm::gpt_4_o();
        OpenAiBackend {
            client: None,
            api_key: None,
            model,
            logging_enabled: true,
            tracing_guard: None,
        }
    }

    fn setup(&mut self) {
        if self.client.is_some() {
            return;
        }
        let api_key = if let Some(api_key) = &self.api_key {
            api_key.to_owned()
        } else {
            if self.logging_enabled {
                tracing::info!("openai_backend api_key not set. Attempting to load from .env");
            } else {
                println!("openai_backend api_key not set. Attempting to load from .env");
            }
            dotenv().ok();
            if let Ok(api_key) = dotenv::var("OPENAI_API_KEY") {
                api_key
            } else {
                if self.logging_enabled {
                    tracing::info!(
                        "OPENAI_API_KEY not fund in in dotenv, nor was it set manually."
                    );
                }
                panic!("OPENAI_API_KEY not fund in in dotenv, nor was it set manually.");
            }
        };

        if self.model.tokenizer.is_none() {
            panic!("Tokenizer did not load correctly.")
        }

        let backoff = backoff::ExponentialBackoffBuilder::new()
            .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
            .build();
        let config = OpenAIConfig::new().with_api_key(api_key);
        self.client = Some(OpenAiClient::with_config(config).with_backoff(backoff));
        // self.tokenizer = Some(LlmTokenizer::new_tiktoken(&self.model.model_id));
    }

    /// Initializes the OpenAiBackend and returns the LlmClient for usage.
    pub fn init(mut self) -> Result<LlmClient> {
        if self.logging_enabled {
            self.tracing_guard = Some(logging::create_logger("openai_backend"));
        }
        self.setup();
        Ok(LlmClient::new(LlmBackend::OpenAi(self)))
    }

    fn client(&self) -> &OpenAiClient<OpenAIConfig> {
        self.client.as_ref().unwrap()
    }

    /// Set the API key for the OpenAI client. Otherwise it will attempt to load it from the .env file.
    pub fn api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }

    /// Set the model for the OpenAI client using the model_id string.
    pub fn from_model_id(mut self, model_id: &str) -> Self {
        self.model = OpenAiLlm::openai_backend_from_model_id(model_id);
        self.model.with_tokenizer();
        self
    }

    /// Use gpt-4 as the model for the OpenAI client.
    pub fn gpt_4(mut self) -> Self {
        self.model = OpenAiLlm::gpt_4();
        self.model.with_tokenizer();
        self
    }

    /// Use gpt-4-32k as the model for the OpenAI client. Limited support for this model from OpenAI.
    pub fn gpt_4_32k(mut self) -> Self {
        self.model = OpenAiLlm::gpt_4_32k();
        self.model.with_tokenizer();
        self
    }

    /// Use gpt-4-turbo as the model for the OpenAI client.
    pub fn gpt_4_turbo(mut self) -> Self {
        self.model = OpenAiLlm::gpt_4_turbo();
        self.model.with_tokenizer();
        self
    }

    /// Use gpt-4-o as the model for the OpenAI client.
    pub fn gpt_4_o(mut self) -> Self {
        self.model = OpenAiLlm::gpt_4_o();
        self.model.with_tokenizer();
        self
    }

    /// Use gpt-3.5-turbo as the model for the OpenAI client.
    pub fn gpt_3_5_turbo(mut self) -> Self {
        self.model = OpenAiLlm::gpt_3_5_turbo();
        self.model.with_tokenizer();
        self
    }

    /// If set to false, will disable logging. By defaults logs to the logs dir.
    pub fn logging_enabled(mut self, logging_enabled: bool) -> Self {
        self.logging_enabled = logging_enabled;
        self
    }

    /// A function to create text completions from a given prompt. Called by various agents, and not meant to be called directly.
    pub async fn text_generation_request(
        &self,
        req_config: &RequestConfig,
        logit_bias: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<String> {
        let prompt = req_config.default_formatted_prompt.as_ref().unwrap();
        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder
            .model(self.model.model_id.to_string())
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(prompt["system"]["content"].clone())
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(prompt["user"]["content"].clone())
                    .build()?
                    .into(),
            ])
            .max_tokens(req_config.actual_request_tokens.unwrap() as u16)
            .frequency_penalty(req_config.frequency_penalty)
            .presence_penalty(req_config.presence_penalty)
            .temperature(req_config.temperature)
            .top_p(req_config.top_p);

        if let Some(logit_bias) = logit_bias {
            request_builder.logit_bias(logit_bias.to_owned());
        }

        let request = request_builder.build()?;
        if self.logging_enabled {
            tracing::info!(?request);
        }
        match self.client().chat().create(request).await {
            Err(e) => {
                let error =
                    anyhow::format_err!("OpenAiBackend text_generation_request error: {}", e);

                if self.logging_enabled {
                    tracing::info!(?error);
                }
                Err(error)
            }
            Ok(completion) => {
                if self.logging_enabled {
                    tracing::info!(?completion);
                }
                if completion.choices.is_empty() {
                    let error = anyhow::format_err!(
                                "OpenAiBackend text_generation_request error: completion.content.is_empty()"
                            );
                    Err(error)
                } else {
                    match &completion.choices[0].message.content {
                        None => {
                            let error = anyhow::format_err!(
                                "OpenAiBackend text_generation_request error: completion.choices[0].message.content.is_none()"
                            );
                            Err(error)
                        }
                        Some(message_content) => {
                            match parse_text_generation_response(message_content) {
                                Err(e) => {
                                    let error = anyhow::format_err!(
                                        "OpenAiBackend text_generation_request error: {}",
                                        e
                                    );
                                    Err(error)
                                }
                                Ok(content) => Ok(content),
                            }
                        }
                    }
                }
            }
        }
    }
}
