//! Actix Web juniper example
//!
//! A simple example integrating juniper in Actix Web

use std::env;
use std::{io, sync::Arc};

use actix_cors::Cors;
use actix_web::{middleware, web::Data, App, HttpServer};
use services_rest::create_frame;

mod schema;
use crate::schema::create_schema;

mod services_graphql;
use crate::services_graphql::{graphql, graphql_playground};

mod services_rest;
use crate::services_rest::get_frames;

use sqlx::sqlite::SqlitePool;

pub mod base64;

pub struct AppState {
    schema: Arc<schema::Schema>,
    db_pool: SqlitePool,
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Create Juniper schema
    let schema = Arc::new(create_schema());

    log::info!("starting HTTP server on port 8080");
    log::info!("GraphiQL playground: http://localhost:8080/graphiql");

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = SqlitePool::connect(&db_url)
        .await
        .expect(format!("Failed to connect to database: {}", &db_url).as_str());

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(AppState {
                schema: schema.clone(),
                db_pool: pool.clone(),
            }))
            .service(graphql)
            .service(graphql_playground)
            .service(get_frames)
            .service(create_frame)
            // the graphiql UI requires CORS to be enabled
            .wrap(Cors::permissive())
            .wrap(middleware::Logger::default())
    })
    .workers(2)
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
