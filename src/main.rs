mod metrics;
mod rpc;

use dotenv::dotenv;
use std::env;

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use tracing_subscriber::EnvFilter;

async fn greet() -> impl Responder {
    HttpResponse::Ok().body("Running!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    openssl_probe::init_ssl_cert_env_vars();
    dotenv().ok();

    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let rpc_service_config = rpc::RpcServiceConfig::from_env();

    let _ = tokio::spawn(rpc::start_service(rpc_service_config));

    HttpServer::new(move || {
        // Configure CORS middleware
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::ACCEPT,
            ])
            .max_age(3600)
            .supports_credentials();

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::new(
                "%{r}a \"%r\"	%s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
            ))
            .wrap(tracing_actix_web::TracingLogger::default())
            .service(metrics::get_metrics)
            .route("/", web::get().to(greet))
    })
    .bind(format!(
        "127.0.0.1:{}",
        env::var("PORT").unwrap_or("3005".to_string())
    ))?
    .run()
    .await?;

    Ok(())
}
