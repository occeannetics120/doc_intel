use std::vec;

use actix_web::{get, http::header::q, post, web };
use serde_json::{ Value};







pub async fn embed_qwen_8b( chunks: &Vec<String>) -> Result<Vec<Vec<f64>>,String>{

      let client =  reqwest::Client::new();
      let qwen_connector = "http://localhost:11434/api/embed";
      let response = client.post(qwen_connector).json(&serde_json::json !({
            "model": "qwen3-embedding:8b",
            "input": chunks
      })).send().await;




      
      match response {
            Ok(resp) =>{
                  let json_resp: Result<serde_json::Value,String> = resp.json().await.map_err(|e| e.to_string()); //owner ship get's transferred to the caller if it's owned (not a ref)
                  match json_resp {
                        Ok(jresp) =>{
                               let s = jresp.to_string();
                              //  println!("jsrep = {:?}",&s[..500]);
                               let vec_array = serde_json::from_value(jresp["embeddings"].clone()).unwrap();
                               Ok(vec_array)
                        }

                        _=>{
                              Err("There was an error unwrapping json vector response ".to_string())
                        }
                  }
            }
            _=>{
                  println!("There was an error ");
                  Err("There was an error generating embed vectors".to_string())
            }
      }

}