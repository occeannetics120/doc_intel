use actix_multipart::form::json;

use crate::api::post_main_query::llmInput;

pub async fn query_llm(input: llmInput) {
    let client = reqwest::Client::new();

    let mut input_concated = input.question;

    let chunks_concated = input.chunks.join("\n\n");

    input_concated = input_concated + &chunks_concated;

    let llm_resp = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
                "model":"qwen2.5:7b",
                "messages": [ {"role":"user","content":input_concated}],
                 "stream": false,
        }))
        .send()
        .await;

    match llm_resp {
        Ok(resp) => {
            let json_string: Result<serde_json::Value, String> =
                resp.json().await.map_err(|e| e.to_string());
            println!("\n \n llm response = {:?}", json_string);
        }

        Err(e) => {
            println!("there was an error with the llm call ,{:?}", e);
        }
    }
}
