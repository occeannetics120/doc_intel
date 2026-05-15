use actix_web::{get, http::header::q, post, web };




pub async  fn embedder(chunks: &Vec<String>){

            embed_qwen_8b(&chunks).await;



} 




async fn embed_qwen_8b( chunks: &Vec<String>) {

      let client =  reqwest::Client::new();
      let qwen_connector = "http://localhost:11434/api/embed";
      let response = client.post(qwen_connector).json(&serde_json::json !({
            "model": "qwen3-embedding:8b",
            "input": chunks
      })).send().await;

      match response {
            Ok(resp) =>{
                  print!("vectors generated : {:?}",resp.json::<serde_json::Value>().await)
            }
            _=>{
                  println!("There was an error ");
            }
      }

}