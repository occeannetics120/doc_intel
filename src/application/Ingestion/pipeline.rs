use actix_web::cookie::time::ext;
use uuid::Uuid;

use crate::application::Ingestion::{ chunker::{self, ChunkInput}, embedder, extractor};




pub struct InjestRequest {
    pub vessel_id: Option<i64>,  
    pub doc_name: Option<String>,
    pub scope_id: Option<Uuid>,
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
                    println!("Was able to extract content ");  
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

        









        //chunk the extracted content 

        // let Ok(chunked_content )= chunk_into_500(extracted_content);







}