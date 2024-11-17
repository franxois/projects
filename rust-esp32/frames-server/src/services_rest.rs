use crate::AppState;
use actix_web::{post, route, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

#[route("/frames", method = "GET")]
pub async fn get_frames() -> impl Responder {
    let frames = [1, 2, 3];
    HttpResponse::Ok().json(frames)
}

#[derive(Serialize, Deserialize, Debug)]
struct CreateFrameRequest {
    name: String,
    mac: String,
    temperature: f32,
}

#[post("/frame")]
pub async fn create_frame(
    st: web::Data<AppState>,
    data: web::Json<CreateFrameRequest>,
) -> impl Responder {
    println!("Received data: {:#?}", data);

    let frames = [1, 2, 3];
    HttpResponse::Ok().json(frames)
}
