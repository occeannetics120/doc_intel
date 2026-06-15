use actix_web::{HttpResponse, Responder, post, web};

use crate::{
    application::{
        Ingestion::embedder::embed_qwen_8b, llm::query_llm::query_llm, main_query::query_qdrant1::{Query1, query_qdrant1}
    },
    main,
};
#[derive(serde::Deserialize)]
pub struct InputMainQuery {
    pub question: String,
    pub chunker_type: String,
}

pub struct llmInput {
    pub question: String,
    pub chunks: Vec<String>,
}

#[post("/query")]
pub async fn post_main_query(main_query: web::Json<InputMainQuery>) -> HttpResponse {
    let mut main_query_embed_input: Vec<String> = Vec::new();

    main_query_embed_input.push(main_query.question.clone());

    let main_query_embed_req = embed_qwen_8b(&main_query_embed_input).await;
    let query_qdrant1_resp;
    let query_res;
    match main_query_embed_req {
        Ok(resp) => {


            query_qdrant1_resp = query_qdrant1(Query1 {
                vectors: resp[0].iter().map(|f| *f as f32).collect(),
                chunker_type: main_query.chunker_type.clone(),
            })
            .await;
            match query_qdrant1_resp {
                Ok(x) => {
                    let llm_query_input = llmInput {
                        question: main_query.question.clone(),
                        chunks: x,
                    };

                     query_res = query_llm(llm_query_input).await;

                     match query_res {
                         Ok(res) =>{
                              HttpResponse::Ok().json(res)
                         }

                         Err(e)=>{
                             HttpResponse::InternalServerError().body(e)
                         }
                     }


                }
                Err(e) => {
                    HttpResponse::InternalServerError().body(e)
                }
            }
        }

        Err(e) => {
            println!("There was an  error embedding question");
            HttpResponse::InternalServerError().body(e)
        }
    }

}
