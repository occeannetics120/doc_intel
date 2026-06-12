use text_splitter::TextSplitter;

use crate::application::Ingestion::embedder::embed_qwen_8b;

pub struct ChunkInput {
    pub text: String,
}

pub fn chunk_into_500(extracted_file: ChunkInput) -> Vec<String> {
    let mut chunk_output: Vec<String> = Vec::new(); // manually initialising compulsary  
    let chunk_input_text = extracted_file.text; //owning only the text field other things can be fully used 

    let mut i = 0;

    let words: Vec<&str> = chunk_input_text.split_whitespace().collect();
    let n = words.len(); // this does copy no borrow is held

    while i < n {
        let start = if i >= 50 { i - 50 } else { 0 };
        let end = (i + 500).min(words.len());

        chunk_output.push(words[start..end].join(" "));
        i = end + 1;
    }

    // println!("\n Chunked output = {:?}" , chunk_output);
    return chunk_output;
}

pub async fn chunk_by_centroid_sentence(chunk_input: ChunkInput) -> Vec<String> {
    let mut chunk_output: Vec<String> = Vec::new();

    let chunk_input_text = chunk_input.text;

    let splitter = TextSplitter::new(500);

    let sentences: Vec<&str> = splitter.chunks(&chunk_input_text).collect();

    let mut i = 0;
    let nsens = sentences.len();
    let mut curr_words = 0;
    let mut curr_string = String::new();
    let mut curr_centroid: Vec<f64> = Vec::new();
    let mut dummy_array_vec: Vec<String> = Vec::new();
    let mut sentence_count_curr: f64 = 0.0;

    while i < nsens {
        let temp_words_vec: Vec<&str> = sentences[i].split_whitespace().collect();

        dummy_array_vec.clear();
        dummy_array_vec.push(sentences[i].to_string());

        let temp_embed_vec_array = embed_qwen_8b(&dummy_array_vec).await;
        let  temp_embed_vec;

        match temp_embed_vec_array {
            Ok(res) => {
                temp_embed_vec = res[0].clone();

                if curr_words + temp_words_vec.len() > 450 {
                    chunk_output.push(curr_string);

                    curr_words = temp_words_vec.len();
                    curr_string = sentences[i].to_string();
                    curr_centroid = temp_embed_vec;
                } else {
                    sentence_count_curr += 1.0;
                    curr_words += temp_words_vec.len();
                    curr_string.push_str(sentences[i]);

                    for i in 0..curr_centroid.len() {
                        curr_centroid[i] = curr_centroid[i] + temp_embed_vec[i];

                        curr_centroid[i] = curr_centroid[i] / sentence_count_curr;
                    }
                }
            }

            _ => {}
        }
        i+=1;


    }

    if curr_string != "" {
        chunk_output.push(curr_string);
    }

    chunk_output
}
