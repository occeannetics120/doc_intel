


use file_type::FileType;
use pdf_extract::{extract_text_from_mem};

use crate::application::Ingestion::chunker::{ChunkInput, chunker};





pub async fn convert_to_string ( raw_file : Vec<u8>)  { 

        let raw_file_type = FileType::from_bytes(&raw_file); //immutable borrow released just after the line 
        
        println!(" raw file type = {:?}",raw_file_type);

        match raw_file_type {
              ft if ft.file_format().name.contains("Portable Document Format") => pdf_extractor(&raw_file).await, //passing an immutable borrow of raw_file so that we can reuse it or pass it down the pipeline or for logs 
              _ => println!("unknown file type"),
        }



}



pub async  fn pdf_extractor (raw_file : &[u8] ){

        let content =  extract_text_from_mem(&raw_file).unwrap();
        println!(" extracted text = {:?}",content);

        chunker(ChunkInput{ text: content}).await;


     

}