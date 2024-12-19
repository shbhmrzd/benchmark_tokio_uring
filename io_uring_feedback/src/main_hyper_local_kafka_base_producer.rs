use crate::main_hyper_local_kafka_base_producer::kafka::set_up_kafka;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{BaseProducer, BaseRecord};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use testcontainers::clients::Cli;
use tokio::time::interval;

#[path = "../../infra/kafka.rs"]
mod kafka;

// Configuration Constants
const MAX_BATCH_SIZE: usize = 1000; // Maximum number of messages per batch
const FLUSH_INTERVAL_MS: u64 = 10; // Kafka flush interval in milliseconds

// Function to flush batched Kafka messages
fn flush_to_kafka(producer: &BaseProducer, topic: &str, messages: Vec<Vec<u8>>) {
    for payload in messages {
        let key = "key".to_string();
        if let Err(e) = producer.send(
            BaseRecord::to(topic)
                .payload(&payload) // &[u8] implements ToBytes
                .key(&key), // String also implements ToBytes
        ) {
            eprintln!("Failed to send message to Kafka: {:?}", e);
        }
    }

    // Poll the producer to process delivery callbacks
    producer.poll(Duration::from_millis(0));
}

// Kafka Batch Sender Task
async fn kafka_batch_sender(
    producer: Arc<BaseProducer>,
    topic: String,
    shared_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
) {
    let mut flush_interval = interval(Duration::from_millis(FLUSH_INTERVAL_MS));

    loop {
        flush_interval.tick().await;

        // Lock the shared buffer and drain messages into a batch
        let mut buffer = shared_buffer.lock().unwrap();
        if buffer.is_empty() {
            continue; // Skip flushing if there are no messages
        }

        let mut messages = Vec::new();
        while let Some(payload) = buffer.pop() {
            messages.push(payload);
            if messages.len() >= MAX_BATCH_SIZE {
                break; // Stop if we reach the batch size
            }
        }

        drop(buffer); // Unlock the buffer early

        // Flush the batch to Kafka
        flush_to_kafka(&producer, &topic, messages);
    }
}

// HTTP Request Handler
async fn handle_request(
    req: Request<Body>,
    shared_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
) -> Result<Response<Body>, Infallible> {
    if req.method() == hyper::Method::POST && req.uri().path() == "/" {
        // Read the body of the HTTP request
        let body_bytes = hyper::body::to_bytes(req.into_body())
            .await
            .unwrap_or_default();
        let payload = body_bytes.to_vec();

        // Push the payload into the shared buffer
        {
            let mut buffer = shared_buffer.lock().unwrap();
            buffer.push(payload);
        }

        // Respond immediately to the HTTP request
        Ok(Response::new(Body::from("Message queued for Kafka")))
    } else {
        Ok(Response::new(Body::from("404 Not Found")))
    }
}

// Main Function
fn main() {
    tokio_uring::start(async {
        let docker = Cli::default();

        // Set up Kafka using testcontainers
        let (bootstrap_servers, _kafka_container, _zk_container) = set_up_kafka(&docker);
        println!("Bootstrap servers: {}", bootstrap_servers);

        // Kafka producer configuration
        let producer: BaseProducer = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers) // Change to your Kafka broker
            .set("message.timeout.ms", "5000") // Timeout for Kafka acknowledgments
            .set("linger.ms", "5") // Enable batching
            .set("batch.size", "65536") // Batch size in bytes (64 KB)
            .set("compression.type", "lz4") // Enable LZ4 compression for higher throughput
            .create()
            .expect("Failed to create Kafka producer");

        // Shared buffer for Kafka messages
        let shared_buffer: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        // Spawn the Kafka Batch Sender Task
        let batch_sender_buffer = shared_buffer.clone();
        let producer_arc = Arc::new(producer);
        tokio_uring::spawn(kafka_batch_sender(
            producer_arc.clone(),
            "base_producer_topic".to_string(),
            batch_sender_buffer,
        ));

        // Set up the HTTP Server
        let make_svc = make_service_fn(move |_| {
            let shared_buffer = shared_buffer.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    handle_request(req, shared_buffer.clone())
                }))
            }
        });

        let addr = ([127, 0, 0, 1], 8080).into();
        let server = Server::bind(&addr).serve(make_svc);

        println!("Running server on http://127.0.0.1:8080");

        // Run the HTTP Server
        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    });
}

/*

const TOTAL_REQUESTS: usize = 1000;

// Adjust Criterion configuration to avoid warnings and control benchmark behavior
fn configure_criterion() -> CriterionConfig {
    CriterionConfig::default()
        .measurement_time(std::time::Duration::from_secs(600))
        .sample_size(15) // Reduce the number of samples to 50 for faster benchmarking
        .warm_up_time(std::time::Duration::from_secs(10)) // Add a warm-up time of 5 seconds to stabilize the server
}


Measured Time	Throughput
Minimum (541.38 ms)	~1846.4 requests/second
Median (27.306 s)	~36.6 requests/second
Maximum (54.348 s)	~18.4 requests/second


Lets optimise by batching the requests to Kafka
*/
