use actix_multipart::form::json;

use crate::api::post_main_query::llmInput;

pub async fn query_llm(input: llmInput) {
    let client = reqwest::Client::new();

    let mut questions = input.question;

    let chunks_concated = input.chunks.join("\n\n");

    let mut context_concated = "The following is the context to answer the question , if the exact answer isnt there , say that you couldnt give the answer  : ".to_string();

    context_concated = context_concated + &chunks_concated;//we had to convert context to_string cause the former is a static and we ahve to make it dyanmic whic in turn allocateds it on heap and gives us the ownership .. 


    let llm_resp = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
                "model":"qwen2.5:7b",
                "messages": [ 
                    {"role" : "system" , "content": context_concated},
                    {"role":"user","content":questions},
                    
                    ],
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
