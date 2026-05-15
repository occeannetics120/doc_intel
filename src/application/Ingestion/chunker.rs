


pub struct ChunkInput{
    pub text: String,
    
}

pub fn chunker( extracted_file : ChunkInput) {


        let mut chunk_output: Vec<String> = Vec::new(); // manually initialising compulsary  
        let chunk_input_text = extracted_file.text; //owning only the text field other things can be fully used 
        
        let mut i = 0;

        let words : Vec<&str> = chunk_input_text.split_whitespace().collect(); 
        let n = words.len(); // this does copy no borrow is held


        while  i  < n {

                let start = if i>= 50 {i-50} else { 0};
                let end =  (i+500).min(words.len());

                chunk_output.push(words[start..end].join(" "));
                i = end+1;
            
        }


        println!("\n Chunked output = {:?}" , chunk_output);
       


}