use actix_web::cookie::time::ext;
use std::fs;
use uuid::Uuid;

use crate::application::Ingestion::{
    chunker::{self, ChunkInput},
    embedder, extractor, vector_dump,
};

pub struct InjestRequest {
    pub vessel_id: Option<i64>,
    pub doc_name: Option<String>,
    pub scope_id: Option<String>,
    pub doc_scope: Option<String>, //vessel , fleet , vessel_type
    pub doc_type: Option<String>,  // incident_report , manual , certificate
    pub file: Vec<u8>,
    //chunk index we will be feeding in this application
}

pub async fn ingest_process(injest_input: InjestRequest) {
    //step 1 extract content from pdf
    let extracted_output = extractor::convert_to_string(&injest_input.file).await; //await this returns to move forward 
    let extracted_content;
    match extracted_output {
        Ok(text) => {
            // println!("Was able to extract content ");
            extracted_content = text;
        }

        Err(e) => {
            println!("There was an error in extractin text from raw file ");
            return;
        }
    }

    let chunker_content = chunker::chunk_by_centroid_sentence(ChunkInput {
        text: extracted_content,
    })
    .await;

    let embed_output = embedder::embed_qwen_8b(&chunker_content).await;
    let mut embed_vector_content;
    let mut chunk_ids: Vec<Uuid> = Vec::new();

    match embed_output {
        Ok(content) => {
            embed_vector_content = content;
            let json_string = serde_json::to_string_pretty(&embed_vector_content).unwrap();
            // fs::write("vector_example.json", json_string).unwrap();
            // println!("vectors generated= {:?}",embed_vector_content);
            chunk_ids = embed_vector_content
                .iter()
                .map(|_| uuid::Uuid::new_v4())
                .collect();
            vector_dump::save_to_qdrant(
                &injest_input,
                &chunker_content,
                embed_vector_content,
                "centroid".to_string(),
                &chunk_ids,
            )
            .await;
        }
        Err(e) => {
            println!("There was an error in embedding the vectors ");
        }
    }

    if (chunk_ids.iter().len() > 0) {
        let mut chunk_index = 0;
        let mut chunk_id_vec = Vec::new();
        let mut chunk_content_sparse: Vec<String> = Vec::new();
        let mut embed_vector_content_sparse  = Vec::new();
        for chunk in chunker_content.clone() {
            let chunker_content_divs = chunker::chunk_into_sparse_256(ChunkInput { text: chunk });

            let embed_output = embedder::embed_sparse_splade(&chunker_content_divs).await;

            match embed_output {
                Ok(content) => {
                    embed_vector_content_sparse.extend(content.clone());
                    
                    for _ in 0..content.len() {
                        chunk_id_vec.push(chunk_ids[chunk_index]);
                        
                    }

                    // chunk_id_vec.push(chunk_ids[chunk_index]);
                    // let json_string = serde_json::to_string_pretty(&embed_vector_content_sparse).unwrap();

                    
                }
                Err(e) => {
                    println!("There was an error in embedding the vectors ");
                }
            }

            chunk_content_sparse.extend(chunker_content_divs);
            

            chunk_index += 1;
        }
        
        
        vector_dump::save_to_qdrant_sparse(
                        &injest_input,
                        &chunk_content_sparse,
                        embed_vector_content_sparse,
                        chunk_id_vec,
        )
                    .await;
    }

    //chunk the extracted content

    // let Ok(chunked_content )= chunk_into_5p00(extracted_content)
}
