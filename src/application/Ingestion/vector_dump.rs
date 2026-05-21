use std::{collections::HashMap, fs};

use crate::application::Ingestion::pipeline::InjestRequest;
use qdrant_client::{
    Qdrant,
    qdrant::{
        CreateCollectionBuilder, Distance, PointStruct, UpsertPointsBuilder, VectorParamsBuilder,
        create_collection_builder,
    },
};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Serialize)]
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
    chunk_id: String,
    payload: QdrantPayload,
    vector: Vec<f64>,
}

pub async fn save_to_qdrant(
    injest_request: &InjestRequest,
    chunks: &Vec<String>,
    vectors: Vec<Vec<f64>>,
) -> Result<(), String> {
    let mut qdrant_insert: Vec<PointStruct> = Vec::new();
    let mut chunk_index = 0;
    for chunk in chunks {
        let payload_json: HashMap<String, serde_json::Value> = HashMap::from([
            ("chunk_text".to_string(), json!(chunk.to_string())),
            ("chunk_index".to_string(), json!(chunk_index)),
            (
                "doc_name".to_string(),
                json!(injest_request.doc_name.clone()),
            ),
            (
                "doc_type".to_string(),
                json!(injest_request.doc_type.clone()),
            ),
            (
                "doc_scope".to_string(),
                json!(injest_request.doc_scope.clone()),
            ),
            ("vessel_id".to_string(), json!(injest_request.vessel_id)),
            (
                "scope_id".to_string(),
                json!(injest_request.scope_id.clone()),
            ),
        ]);

        let qdrant_payload: HashMap<String, qdrant_client::qdrant::Value> =
            qdrant_client::Payload::from(payload_json).into();

        let vector_f32 : Vec<f32> = vectors[chunk_index].iter().map(|f| *f as f32).collect();
        let qdrant_item: PointStruct = PointStruct {
            id: Some(uuid::Uuid::new_v4().to_string().into()),
            payload: qdrant_payload.into(),
            vectors: Some(vector_f32.clone().into()),
        };
        qdrant_insert.push(qdrant_item);
        chunk_index += 1;
    }

    // fs::write("vector_dump.json",serde_json::to_string_pretty(&qdrant_insert).unwrap()).unwrap();

    let qdrant_client = Qdrant::from_url("http://76.13.242.204:6334")
        .build()
        .map_err(|e| e.to_string())?;

    // let create_collection_response = qdrant_client.create_collection(
    //     CreateCollectionBuilder::new("test_collection")
    //     .vectors_config( VectorParamsBuilder::new(4096, Distance::Cosine))
    // ).await;

    // match create_collection_response {
    //     Ok(resp) => {
    //         println!("Creating collection was successful");
    //     }

    //     Err(e) =>{
    //         println!("there was an error {}" ,e);
    //     }
    // }

    let insert_resp = qdrant_client
        .upsert_points(UpsertPointsBuilder::new("test_collection", qdrant_insert).wait(true))
        .await;

    match insert_resp {
        Ok(resp) => {
            println!("qdrant insert was successful");
        }

        Err(e) => {
            println!("there was an error {:?} ", e);
        }
    }

    Ok(())
    //  while Some(injest_request)
}
