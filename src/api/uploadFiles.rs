
use actix_multipart::{Multipart};
use actix_web::{post,Responder};
use futures_util::{StreamExt};

use crate::application::Ingestion::extractor::convert_to_string;

#[post("/upload")]
pub async fn upload(mut payload: Multipart ) -> impl Responder{
            while let Some(Ok(mut field)) = payload.next().await {
                let mut bytes = Vec::new(); // upload owns the bytes memory

                while let Some(Ok(chunk)) = field.next().await{
                    bytes.extend_from_slice(&chunk); // bytes is being mut borrowed and released immediately 
                }

                println!("File size: {} kb",(bytes.len()/1000));  //
                convert_to_string(bytes); //owner ship is moved since bytes doesnt have the copy trait 


            }
            
            "all files received"


}