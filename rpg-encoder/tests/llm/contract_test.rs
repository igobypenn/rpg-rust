#![cfg(feature = "llm")]

use rpg_encoder::{LlmConfig, OpenAIClient};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct TestRequest {
    message: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TestResponse {
    result: String,
}

#[test]
#[ignore = "Set OPENAI_API_KEY and run with --ignored to test real API"]
fn test_openai_chat_completion() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let config = LlmConfig::from_env().expect("Failed to load config from env");
        let client = OpenAIClient::new(config).expect("Failed to create client");

        let result = client
            .complete("You are a helpful assistant.", "Say 'hello' in one word.")
            .await
            .expect("API call failed");

        assert!(!result.is_empty());
        println!("Response: {}", result);
    });
}

#[test]
#[ignore = "Set OPENAI_API_KEY and run with --ignored to test real API"]
fn test_openai_json_response() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let config = LlmConfig::from_env().expect("Failed to load config from env");
        let client = OpenAIClient::new(config).expect("Failed to create client");

        #[derive(Debug, Deserialize)]
        struct SimpleResponse {
            word: String,
        }

        let result: SimpleResponse = client
            .complete_json(
                "You are a helpful assistant that responds with JSON.",
                "Return a JSON object with a single field 'word' containing the word 'test'.",
            )
            .await
            .expect("JSON API call failed");

        assert_eq!(result.word.to_lowercase(), "test");
    });
}

#[test]
#[ignore = "Set OPENAI_API_KEY with z.ai key and OPENAI_BASE_URL, then run with --ignored"]
fn test_openai_compatible_zhipu() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4".to_string());
        let model = std::env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "glm-4-flash".to_string());

        let config = LlmConfig::openai_compatible(&base_url, &model);
        let client = OpenAIClient::new(config).expect("Failed to create client");

        let result = client
            .complete("You are a helpful assistant.", "Say 'hello' in Chinese.")
            .await
            .expect("z.ai API call failed");

        assert!(!result.is_empty());
        println!("z.ai Response: {}", result);
    });
}
