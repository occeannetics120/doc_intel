use qdrant_client::{Qdrant, qdrant::QueryPointsBuilder};


pub struct Query1 {
    // payload: {

    // }
    pub vectors : Vec<f32>,
    pub chunker_type: String
}


pub async fn query_qdrant1(question : Query1) -> Result<Vec<String>,String> {

        let qdrant_client = Qdrant::from_url("http://76.13.242.204:6334")
        .build()
        .map_err(|e| e.to_string())?;

        let mut res_chunks : Vec<String> = Vec::new();

        let mut collection_name = "test_collection";

        if(question.chunker_type == "centroid".to_string()){
            collection_name = "test_collection_centroid";
        }

        let query_resp = qdrant_client.query(
            QueryPointsBuilder::new(collection_name)
            .query(question.vectors)
            .with_payload(true)
            .limit(5)
        ).await;


        match query_resp {
            Ok(resp) => {
                println!("Resp = {:?}",resp);
                for point in resp.result {
                    if let Some(chunk_text) = point.payload.get("chunk_text") {
                        res_chunks.push(chunk_text.to_string());
                    }
                }
            }

            Err(e) => {
                println!("There was a error searching for the string {:?}", e);
            }
        }



        Ok(res_chunks)



}