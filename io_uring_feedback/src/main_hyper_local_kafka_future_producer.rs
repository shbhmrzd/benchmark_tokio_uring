use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::convert::Infallible;

use crate::main_hyper_local_kafka_future_producer::kafka::set_up_kafka;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use testcontainers::clients::Cli;

#[path = "../../infra/kafka.rs"]
mod kafka;

// Configuration Constants
const MAX_BATCH_SIZE: usize = 1000; // Maximum number of messages per batch
const FLUSH_INTERVAL_MS: u64 = 10; // Kafka flush interval in milliseconds

// Function to flush batched Kafka messages
async fn flush_to_kafka(producer: &FutureProducer, topic: &str, messages: Vec<Vec<u8>>) {
    for payload in messages {
        let record: FutureRecord<'_, (), Vec<u8>> = FutureRecord::to(topic).payload(&payload);
        if let Err(e) = producer.send(record, rdkafka::util::Timeout::Never).await {
            eprintln!("Failed to send message to Kafka: {:?}", e);
        }
    }
}

// Kafka Batch Sender Task
async fn kafka_batch_sender(
    producer: FutureProducer,
    topic: &str,
    shared_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(FLUSH_INTERVAL_MS));

    loop {
        interval.tick().await;

        // Lock the shared buffer and drain messages for batching
        let mut buffer = shared_buffer.lock().unwrap();
        if buffer.is_empty() {
            continue; // Skip this interval if there's nothing to flush
        }

        let mut messages = Vec::new();
        while let Some(payload) = buffer.pop() {
            messages.push(payload);
            if messages.len() >= MAX_BATCH_SIZE {
                break; // Stop if we reach the batch size
            }
        }

        drop(buffer); // Unlock the buffer early

        // Flush the batch of messages to Kafka
        flush_to_kafka(&producer, topic, messages).await;
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

        // Kafka Producer Configuration
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers)
            .set("message.timeout.ms", "5000") // Timeout for Kafka acknowledgments
            .set("linger.ms", "5") // Enable batching
            .set("batch.size", "65536") // Batch size in bytes (64 KB)
            .set("compression.type", "lz4") // Enable LZ4 compression for higher throughput
            .create()
            .expect("Failed to create Kafka producer");

        // Shared Buffer for Kafka Messages
        let shared_buffer: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        // Spawn the Kafka Batch Sender Task
        let batch_sender_buffer = shared_buffer.clone();
        tokio_uring::spawn(kafka_batch_sender(
            producer.clone(),
            "future_producer_topic", // Kafka topic name
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

Benchmarking io_uring_feedback_throughput_benchmark: Warming up for 10.000 s


Benchmarking io_uring_feedback_throughput_benchmark: Collecting 15 samples in estimated 601.48 s (30 iterations)




io_uring_feedback_throughput_benchmark
                        time:   [28.247 s 45.834 s 59.792 s]
                        change: [-1.3701% +67.777% +204.41%] (p = 0.08 > 0.05)
                        No change in performance detected.


io_uring_feedback_throughput_benchmark
                        time:   [27.359 s 36.392 s 45.210 s]
                        change: [-49.577% -20.601% +31.492%] (p = 0.34 > 0.05)
                        No change in performance detected.
Found 5 outliers among 15 measurements (33.33%)
  3 (20.00%) low severe
  1 (6.67%) high mild
  1 (6.67%) high severe

################################################
Let's try with some kafka producer optimisations
################################################

Benchmarking io_uring_feedback_throughput_benchmark: Warming up for 5.0000 s
Warning: Unable to complete 11 samples in 300.0s. You may wish to increase target time to 504.7s, or reduce sample count to 10.
Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated 504.71 s (11 iterations)


io_uring_feedback_throughput_benchmark
                        time:   [331.48 ms 36.835 s 73.350 s]
                        change: [-98.706% +1.2181% +118.05%] (p = 0.98 > 0.05)
                        No change in performance detected.

## Benchmark Results

| Metric      | Sample Time (s) | Throughput (req/s) |
|-------------|------------------|--------------------|
| **Minimum** | 0.33148          | **3016.26 req/s**  |
| **Median**  | 36.835           | **27.15 req/s**    |
| **Maximum** | 73.350           | **13.63 req/s**    |

---
*/
