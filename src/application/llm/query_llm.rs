use actix_multipart::form::json;
use actix_web::cookie::time::{UtcDateTime, macros::datetime};

use crate::{api::post_main_query::llmInput, application::llm};

#[derive(serde::Serialize)]
pub struct LlmMessage { 
    pub role : String,
    pub content : String
}


#[derive(serde::Serialize)]
pub struct LlmResponseTiming{
    pub eval_count : i64,
    pub load_duration : f64,
    pub eval_duration : f64,
}

#[derive(serde::Serialize)]
pub struct LlmPromptTiming{
    pub prompt_eval_count : i64,
    pub prompt_eval_duration : f64,
     pub total_duration : f64,

}

#[derive(serde::Serialize)]
pub struct LlmOutput{
    pub status : String,
    pub model : String,
    pub created_at: String,
    pub done : bool,
    pub done_reason: String,
    pub message : LlmMessage,
    pub prompt_metrics : LlmPromptTiming,
    pub response_metrics : LlmResponseTiming,
}

pub async fn query_llm(input: llmInput) -> Result<LlmOutput,String>{
    let client = reqwest::Client::new();

    let  questions = input.question;

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


            // println!("\n \n llm response = {:?}", json_string);
            println!("{}", serde_json::to_string_pretty(&json_string).unwrap());
            let parsed = json_string.unwrap();

            let llm_message = LlmMessage{
                role : parsed["message"]["role"].as_str().unwrap().to_string(),
                content: parsed["message"]["content"].as_str().unwrap().to_string(),
            };
            println!("parsed llm message");
            let llm_prompt_metrics = LlmPromptTiming{
                     prompt_eval_count : parsed["prompt_eval_count"].as_i64().unwrap(),
                     prompt_eval_duration : parsed["prompt_eval_duration"].as_f64().unwrap()/f64::powi(10.0, 6),
                     total_duration : parsed["total_duration"].as_f64().unwrap()/f64::powi(10.0, 6)
            };
            println!("parsed llm prompt metrics");
            let llm_response_metrics = LlmResponseTiming{
                     eval_count : parsed["eval_count"].as_i64().unwrap(),
                     eval_duration : parsed["eval_duration"].as_f64().unwrap()/f64::powi(10.0, 6),
                     load_duration : parsed["load_duration"].as_f64().unwrap()/f64::powi(10.0, 6)
            };
            println!("parsed llm response metrics");



            let llm_output = LlmOutput{
                status : "ok".to_string(), 
                model : parsed["model"].as_str().unwrap().to_string(),
                created_at: parsed["created_at"].as_str().unwrap().to_string(),
                done : parsed["done"].as_bool().unwrap(),
                done_reason: parsed["done_reason"].as_str().unwrap().to_string(),
                message: llm_message,
                prompt_metrics: llm_prompt_metrics,
                response_metrics: llm_response_metrics
                
            };

            println!("parsed llm output");

            Ok(llm_output)
        }

        Err(e) => {
            println!("there was an error with the llm call ,{:?}", e);
            Err("There was an error".to_string())
            
        }
    }

}
