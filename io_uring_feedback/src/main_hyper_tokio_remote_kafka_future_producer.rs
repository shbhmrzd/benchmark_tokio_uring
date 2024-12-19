use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_uring::start as tokio_uring_start;

// Kafka topic
const KAFKA_TOPIC: &str = "sessionlet-completion-tlb2-aa-scalable-pt1m-dev";

// HTTP request handler
async fn handle_request(
    req: Request<Body>,
    producer: Arc<FutureProducer>,
) -> Result<Response<Body>, Infallible> {
    if req.method() == hyper::Method::POST && req.uri().path() == "/" {
        // Extract the body payload
        let body_bytes = hyper::body::to_bytes(req.into_body())
            .await
            .unwrap_or_default();
        let payload = body_bytes.to_vec();

        // Send the payload to Kafka via Tokio-Uring
        let producer_clone = producer.clone();
        tokio_uring_start(async move {
            let record: FutureRecord<'_, (), Vec<u8>> =
                FutureRecord::to(KAFKA_TOPIC).payload(&payload);

            if let Err(e) = producer_clone
                .send(record, rdkafka::util::Timeout::Never)
                .await
            {
                eprintln!("Failed to send message to Kafka: {:?}", e);
            }
        });

        // Respond immediately to the HTTP request
        Ok(Response::new(Body::from("Message sent to Kafka")))
    } else {
        Ok(Response::new(Body::from("404 Not Found")))
    }
}

// Main function
#[tokio::main]
async fn main() {
    // Kafka producer configuration
    let producer: FutureProducer = ClientConfig::new()
        .set(
            "bootstrap.servers",
            "rccp103-9e.iad3.prod.conviva.com:32300",
        )
        .set("message.timeout.ms", "5000") // Kafka message timeout
        .set("linger.ms", "5") // Wait 5ms for batching
        .set("batch.size", "65536") // Batch size in bytes (64KB)
        .set("compression.type", "lz4") // Enable LZ4 compression for better throughput
        .create()
        .expect("Failed to create Kafka producer");

    // Wrap the producer in an Arc for shared ownership
    let producer_arc = Arc::new(producer);

    // Set up the HTTP server
    let make_svc = make_service_fn(move |_| {
        let producer_clone = producer_arc.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, producer_clone.clone())
            }))
        }
    });

    let addr = ([127, 0, 0, 1], 8080).into();
    let server = Server::bind(&addr).serve(make_svc);

    println!("Running server on http://127.0.0.1:8080");

    // Run the HTTP server
    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
}

/*
kafkacat -b rccp103-9e.iad3.prod.conviva.com:32300 -t sessionlet-completion-tlb2-aa-scalable-pt1m-dev -C -o end

hread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/runtime/mod.rs:130:14:
Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
thread 'tokio-runtime-worker' panicked at thread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/lib.rs:148:48:
called `Result::unwrap()` on an `Err` value: Os { code: 24, kind: Uncategorized, message: "Too many open files" }
/root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/lib.rs:148:48:
called `Result::unwrap()` on an `Err` value: Os { code: 24, kind: Uncategorized, message: "Too many open files" }
thread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/lib.rs:148:48:
called `Result::unwrap()` on an `Err` value: Os { code: 24, kind: Uncategorized, message: "Too many open files" }
thread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/lib.rs:148:48:

Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks.
thread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/runtime/mod.rs:130:14:
Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks.
thread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/runtime/mod.rs:130:14:
Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks.
thread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/runtime/mod.rs:130:14:
Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks.
thread 'tokio-runtime-worker' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-uring-0.5.0/src/runtime/mod.rs:130:14:
Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks.

*/
