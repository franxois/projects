use actix_web::{route, HttpResponse, Responder};

#[route("/frames", method = "GET")]
pub async fn get_frames() -> impl Responder {
    let frames = [1, 2, 3];
    HttpResponse::Ok().json(frames)
}
