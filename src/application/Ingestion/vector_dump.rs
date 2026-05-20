use std::fs;

use actix_web::http::header::QualityItem;
use uuid::Uuid;

use crate::application::Ingestion::pipeline::InjestRequest;
use 

pub struct QdrantPayload {
    chunk_text: String,
    chunk_index: usize,
    doc_name: Option<String>,
    doc_type: Option<String>,
    doc_scope: Option<String>,
    vessel_id: Option<i64>,
    scope_id: Option<String>,
}

#[derive(Serialize)]
pub struct QdrantItem {
    chunk_id: Uuid,
    payload: QdrantPayload,
    vector: Vec<f32>,
}


pub async fn save_to_qdrant(
    injest_request: &InjestRequest,
    chunks: &Vec<String>,
    vectors: Vec<Vec<f32>>,
) {
    let mut qdrant_insert: Vec<QdrantItem> = Vec::new();
    let mut chunk_index = 0;
    for chunk in chunks {
        let mut qdrant_payload: QdrantPayload = QdrantPayload {
            chunk_text: chunk.to_string(),
            chunk_index: (chunk_index),
            doc_name: injest_request.doc_name.clone(),
            doc_type: injest_request.doc_type.clone(),
            doc_scope: injest_request.doc_scope.clone(),
            vessel_id: injest_request.vessel_id,
            scope_id: injest_request.scope_id.clone(),
        };

        let mut qdrant_item: QdrantItem = QdrantItem {
            chunk_id: uuid::Uuid::new_v4(),
            payload:qdrant_payload,
            vector: vectors[chunk_index].clone(),
        };
        qdrant_insert.push(qdrant_item);
        chunk_index+=1;



        
    }


    fs::write("vector_dump.json",serde_json::to_string_pretty(&qdrant_insert).unwrap()).unwrap();



    //  while Some(injest_request)
}
