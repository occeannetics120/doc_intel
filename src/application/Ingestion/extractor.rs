


use actix_web::Result;
use file_type::FileType;
use lopdf::content::Content;
use pdf_extract::{extract_text_from_mem};







pub async fn convert_to_string ( raw_file : &Vec<u8>)  -> Result<String,String> { 

        let raw_file_type = FileType::from_bytes(raw_file); //immutable borrow released just after the line 
        
        println!(" raw file type = {:?}",raw_file_type);

        match raw_file_type {
              ft if ft.file_format().name.contains("Portable Document Format") => pdf_extractor(raw_file).await, //passing an immutable borrow of raw_file so that we can reuse it or pass it down the pipeline or for logs 
              _ => Err("Unknown File Type".to_string()),  //we need to convert this to owned string since that's what the Result expects . 
        }



}



pub async fn pdf_extractor (raw_file : &[u8] ) -> Result<String,String> {

        let content =  extract_text_from_mem(&raw_file).map_err(|e| e.to_string())?; //This question marks results in early return if error 
        println!(" extracted text = {:?}",content);

        Ok(content)
        // chunker(ChunkInput{ text: content}).await;


     

}