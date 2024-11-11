use actix_web::{
    get, route,
    web::{self},
    HttpResponse, Responder,
};
use juniper::http::{graphiql::graphiql_source, GraphQLRequest};

use crate::AppState;

/// GraphiQL playground UI
#[get("/graphiql")]
pub async fn graphql_playground() -> impl Responder {
    web::Html::new(graphiql_source("/graphql", None))
}

/// GraphQL endpoint
#[route("/graphql", method = "GET", method = "POST")]
pub async fn graphql(st: web::Data<AppState>, data: web::Json<GraphQLRequest>) -> impl Responder {
    let user = data.execute(&st.schema, &()).await;
    HttpResponse::Ok().json(user)
}
