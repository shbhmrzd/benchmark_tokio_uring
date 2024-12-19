#[path = "../../infra/app_error.rs"]
mod app_error;
#[path = "../../infra/kafka.rs"]
mod kafka;

use crate::app_error::AppError;
use crate::kafka::set_up_kafka;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use testcontainers::clients::Cli;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize)]
struct Payload {
    data: String,
}

async fn handle_request(
    req: Request<Body>,
    producer: Arc<FutureProducer>,
    last_payload: Arc<Mutex<Option<String>>>,
) -> Result<Response<Body>, AppError> {
    if req.method() == hyper::Method::POST {
        // Parse the incoming payload
        let whole_body = hyper::body::to_bytes(req.into_body()).await?;
        let payload: Payload = serde_json::from_slice(&whole_body)?;

        // Retrieve the previous payload (if any)
        let mut previous_payload = last_payload.lock().await;
        if let Some(prev) = previous_payload.clone() {
            // Send the previous payload to Kafka
            let record = FutureRecord::to("test_topic_tokio")
                .payload(&prev)
                .key("key");
            let _ = producer.send(record, rdkafka::util::Timeout::Never).await;
        }

        // Store the current payload for the next request
        *previous_payload = Some(payload.data.clone());

        Ok(Response::new(Body::from("Payload received")))
    } else {
        Ok(Response::new(Body::from("Only POST is supported")))
    }
}

#[tokio::main]
async fn main() {
    // Initialize Docker client
    let docker = Cli::default();

    // Set up Kafka and get the bootstrap servers
    let (bootstrap_servers, _kafka_container, _zk_container) = set_up_kafka(&docker);

    // Kafka producer setup
    let producer = Arc::new(
        ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers)
            .create::<FutureProducer>()
            .expect("Failed to create Kafka producer"),
    );

    // Store the previous payload in memory
    let last_payload = Arc::new(Mutex::new(None::<String>));

    // Define HTTP service
    let make_svc = make_service_fn(move |_| {
        let producer = producer.clone();
        let last_payload = last_payload.clone();

        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                handle_request(req, producer.clone(), last_payload.clone())
            }))
        }
    });

    // Create HTTP server
    let addr = ([127, 0, 0, 1], 8080).into();
    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);
    println!("Bootstrap servers - {}", bootstrap_servers);

    // Run server
    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}

// curl -X POST http://127.0.0.1:8080 -H "Content-Type: application/json" -d '{"data": "your_message_here"}'

// docker ps
// CONTAINER ID   IMAGE                              COMMAND                  CREATED         STATUS         PORTS                                                                       NAMES
// 9a2fd3758fff   wurstmeister/kafka:latest          "start-kafka.sh"         2 minutes ago   Up 2 minutes   0.0.0.0:55003->9094/tcp
//                                                     festive_mendel
// kcat -b localhost:55003 -t test_topic_tokio -C -o beginning
