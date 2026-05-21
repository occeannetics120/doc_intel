use actix_web::cookie::time::ext;
use uuid::Uuid;
use std::fs;

use crate::application::Ingestion::{ chunker::{self, ChunkInput}, embedder, extractor, vector_dump};




pub struct InjestRequest {
    pub vessel_id: Option<i64>,  
    pub doc_name: Option<String>,
    pub scope_id: Option<String>,
    pub doc_scope: Option<String>,  //vessel , fleet , vessel_type 
    pub doc_type: Option<String>, // incident_report , manual , certificate 
    pub file: Vec<u8>,
    //chunk index we will be feeding in this application 
}



pub async  fn ingest_process(injest_input : InjestRequest){
        
        //step 1 extract content from pdf 
        let extracted_output = extractor::convert_to_string(&injest_input.file).await; //await this returns to move forward 
        let extracted_content ;
        match extracted_output  {
             Ok(text)=>{
                    // println!("Was able to extract content ");  
                    extracted_content = text;  
            }

            Err(e)=>{
                  println!("There was an error in extractin text from raw file ");
                  return ;
            }
        }



        let chunker_content = chunker::chunk_into_500(ChunkInput{
            text: extracted_content,
        });



        let embed_output = embedder::embed_qwen_8b(&chunker_content).await;
        let embed_vector_content ;

        match embed_output {
            Ok(content) =>{
                embed_vector_content = content;
                let json_string = serde_json::to_string_pretty(&embed_vector_content).unwrap();
                // fs::write("vector_example.json", json_string).unwrap();
                // println!("vectors generated= {:?}",embed_vector_content);
                vector_dump::save_to_qdrant(&injest_input,&chunker_content,embed_vector_content).await;
            }
            Err(e) =>{
                println!("There was an error in embedding the vectors ");
            }
        }






       












        









        //chunk the extracted content 

        // let Ok(chunked_content )= chunk_into_500(extracted_content);







}