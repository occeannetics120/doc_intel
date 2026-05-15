use actix_web::{App, HttpServer, Responder, get, web};
mod api;
mod application;

#[get("/")]
async fn index() -> impl Responder{
    "Hello World!"
}

#[get("/{name}")]
async fn hello(name: web::Path<String>) -> impl Responder{
    format!("Hello {}!", &name)
}

#[actix_web::main]
async fn main() -> std::io::Result<()>{

    let reqwest_client = reqwest::Client::new(); 

    HttpServer::new(move || App::new()
    .service(index)
    .service(api::uploadFiles::upload)
    .service(hello)
    .app_data(web::Data::new(reqwest_client.clone()))
    )
    
    .bind(("127.0.0.1",8080))?.run().await
}
