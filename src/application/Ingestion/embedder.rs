use std::vec;

use actix_web::{get, http::header::q, post, web};
use serde_json::Value;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SparseVecModel {
    pub values: Vec<f32>,
    pub indices: Vec<u32>,
}

pub async fn embed_qwen_8b(chunks: &Vec<String>) -> Result<Vec<Vec<f64>>, String> {
    let client = reqwest::Client::new();
    let qwen_connector = "http://localhost:11434/api/embed";
    let response = client
        .post(qwen_connector)
        .json(&serde_json::json !({
              "model": "qwen3-embedding:8b",
              "input": chunks
        }))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let json_resp: Result<serde_json::Value, String> =
                resp.json().await.map_err(|e| e.to_string()); //owner ship get's transferred to the caller if it's owned (not a ref)
            match json_resp {
                Ok(jresp) => {
                    let s = jresp.to_string();
                    //  println!("jsrep = {:?}",&s[..500]);
                    let vec_array = serde_json::from_value(jresp["embeddings"].clone()).unwrap();
                    Ok(vec_array)
                }

                _ => Err("There was an error unwrapping json vector response ".to_string()),
            }
        }
        _ => {
            println!("There was an error ");
            Err("There was an error generating embed vectors , embedder.rs".to_string())
        }
    }
}

pub async fn embed_sparse_splade(chunks: &Vec<String>) -> Result<Vec<SparseVecModel>, String> {
    let client = reqwest::Client::new();
    let qwen_connector = "http://76.13.242.204:8000/sparse-embeddings";

    let mut payload_structure = Vec::new();
    for chunk in chunks {
        payload_structure.push(serde_json::json !({"text": chunk}));
    }
    let response = client
        .post(qwen_connector)
        .json(&payload_structure)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let json_resp: Result<serde_json::Value, String> =
                resp.json().await.map_err(|e| e.to_string()); //owner ship get's transferred to the caller if it's owned (not a ref)
            match json_resp {
                Ok(jresp) => {
                    let s = jresp.to_string();
                  //   println!("jsrep = {:?}", &s[..3000]);
                    let vec_array: Vec<SparseVecModel> =
                        serde_json::from_value(jresp.clone()).unwrap(); //unwrap meaning say that it will be a value //deserialize with from_value 
                    Ok(vec_array)
                }

                _ => Err("There was an error unwrapping json vector response ".to_string()),
            }
        }
        _ => {
            println!("There was an error ");
            Err("There was an error generating embed vectors , embedder.rs".to_string())
        }
    }
}
