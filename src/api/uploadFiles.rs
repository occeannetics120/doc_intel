use actix_multipart::Multipart;
use actix_web::{Responder, post};
use futures_util::{StreamExt, lock::Mutex};
use uuid::Uuid;

use crate::application::Ingestion::pipeline::{self, InjestRequest};



#[post("/upload")]
pub async fn upload(mut payload: Multipart) -> impl Responder { //payload is mut cause cursor stays inside and it needs to be changed inorder to iterate over it 


    let  extracted_request: InjestRequest;
    let mut vessel_id: Option<i64> = None;
    let mut doc_name: Option<String> = None;
    let mut scope_id: Option<Uuid> = None;
    let mut doc_scope: Option<String> = None;
    let mut doc_type: Option<String> = None;
    let mut file_bytes : Option<Vec<u8>> = None;

    
    while let Some(Ok(mut field)) = payload.next().await { // we have Some and Ok here because payload.next().await returns two layers of wrapping Option<Result<Field,MultipartError>>

        let field_name = field.name()
                            .map(|s| s.to_owned()); // this is copy and the borrow is closed after the line
        match field_name.as_deref() {
            Some("vessel_id") =>{
                 let mut bytes = Vec::new();
                 while let Some(Ok(chunk)) = field.next().await {
                    bytes.extend_from_slice(&chunk);
                 }

                 vessel_id = String::from_utf8(bytes).ok().and_then(|s| s.trim().parse::<i64>().ok());
             }

            Some(name @ ("doc_name" | "doc_scope" | "doc_type" )) =>{
                 let mut bytes = Vec::new();
                 while let Some(Ok(chunk)) = field.next().await {
                    bytes.extend_from_slice(&chunk);
                 }

                 let value = String::from_utf8(bytes).ok();
                 match name {
                      "doc_name"  => doc_name  = value,
                      "doc_scope" => doc_scope = value,
                      "doc_type"  => doc_type  = value,
                      _ => {

                      }
                 }
             }

             Some("scope_id") =>{
                 let mut bytes = Vec::new();
                 while let Some(Ok(chunk)) = field.next().await {
                    bytes.extend_from_slice(&chunk);
                 }

                 let value = String::from_utf8(bytes).ok().and_then(|s| s.trim().parse::<Uuid>().ok());
                 scope_id = value;
             }

             Some("file") =>{   
                    let mut bytes = Vec::new();
                    while let Some(Ok(chunk)) = field.next().await {
                        bytes.extend_from_slice(&chunk);
                    }

                    file_bytes = Some(bytes);

             }
              

             _=>{

             }
        }

        // let mut bytes = Vec::new(); // upload owns the bytes memory

        // while let Some(Ok(chunk)) = field.next().await {
        //     bytes.extend_from_slice(&chunk); // bytes is being mut borrowed and released immediately 
        // }

        // println!("File size: {} kb", (bytes.len() / 1000)); //
        // convert_to_string(bytes).await; //ownership is moved since bytes doesnt have the copy trait 
    }

    extracted_request = InjestRequest{
        vessel_id,
        doc_name,
        scope_id ,
        doc_scope,
        doc_type, 
        file:file_bytes.unwrap(),
    };

    pipeline::ingest_process(extracted_request);

    "all files received"
}
