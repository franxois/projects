use crate::base64::base64;
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
    #[serde(with = "base64")]
    payload: Vec<u8>,
}

#[post("/frame")]
pub async fn create_frame(
    st: web::Data<AppState>,
    data: web::Json<CreateFrameRequest>,
) -> impl Responder {
    let new_frame = sqlx::query!(
        "INSERT INTO frames (name, mac, temperature, payload)
        VALUES ($1, $2, $3, $4) RETURNING id",
        data.name,
        data.mac,
        data.temperature,
        data.payload
    )
    .fetch_one(&st.db_pool)
    .await
    .unwrap();

    println!(
        "Created frame with id: {} - temp {} : {}",
        new_frame.id, data.name, data.temperature
    );

    HttpResponse::Ok().json(new_frame.id)
}
