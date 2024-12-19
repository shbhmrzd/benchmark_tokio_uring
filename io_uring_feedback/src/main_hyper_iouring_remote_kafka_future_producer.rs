use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::convert::Infallible;
use std::sync::Arc;

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

        // Handle Kafka publishing asynchronously
        let producer_clone = producer.clone();
        tokio_uring::spawn(async move {
            // Send the payload to Kafka
            let binding = payload.clone();
            let record: FutureRecord<'_, (), Vec<u8>> =
                FutureRecord::to(KAFKA_TOPIC).payload(&binding);
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
fn main() {
    tokio_uring::start(async {
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
    });
}

/*
thread 'main' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.42.0/src/task/local.rs:418:29:
`spawn_local` called from outside of a `task::LocalSet` or LocalRuntime
thread 'main' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.42.0/src/task/local.rs:418:29:
`spawn_local` called from outside of a `task::LocalSet` or LocalRuntime
thread 'main' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.42.0/src/task/local.rs:418:29:
`spawn_local` called from outside of a `task::LocalSet` or LocalRuntime
thread 'main' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.42.0/src/task/local.rs:418:29:
`spawn_local` called from outside of a `task::LocalSet` or LocalRuntime
thread 'main' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.42.0/src/task/local.rs:418:29:
`spawn_local` called from outside of a `task::LocalSet` or LocalRuntime
thread 'main' panicked at /root/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.42.0/src/task/local.rs:418:29:



*/

/*
=============
GPT response
=============

The error you are encountering (`spawn_local called from outside of a task::LocalSet or LocalRuntime`) occurs because `tokio_uring::spawn` is being called outside of the expected runtime context. The `tokio_uring` runtime is not compatible with `tokio::spawn_local` directly, and `tokio_uring::spawn` requires that the task be executed within the context of the **`tokio_uring` runtime**.

Let’s debug and resolve the issue step-by-step.

---

### **Problem Diagnosis**
1. **`tokio_uring::spawn` Issue:**
   - The `tokio_uring` runtime is not designed to work seamlessly with standard `tokio` async tasks (`tokio::spawn` or `tokio::spawn_local`).
   - When you call `tokio_uring::spawn` in your code, it assumes you are operating entirely within the `tokio_uring` runtime. However, in your code, the `hyper` HTTP server is using a standard **`tokio` runtime** internally.

   This runtime mismatch causes the `spawn_local` panic.

2. **`tokio::spawn_local` Incompatibility:**
   - The panic is triggered because `tokio::spawn_local` is used under the hood by `tokio_uring`, but the HTTP server (`hyper`) does not run on a **`LocalSet`**, which is required by `tokio::spawn_local`.

3. **`tokio_uring` and `hyper` Conflict:**
   - `tokio_uring` is a special runtime optimized for `io_uring`, which uses low-level kernel I/O features. However, `hyper` relies on the standard `tokio` runtime for async I/O.
   - Mixing `tokio_uring` and `hyper` directly can cause runtime-related issues like this.

---

### **Solution**

To fix the issue, you have a few options:

---

#### **Option 1: Use `tokio` Instead of `tokio_uring`**
If you don’t specifically need `tokio_uring`'s low-level performance benefits for this application, the simplest solution is to replace `tokio_uring::start` with the standard `tokio` runtime. The standard `tokio` runtime is fully compatible with both `hyper` and `rdkafka`.

Update your `main` function to:

```rust
#[tokio::main] // Use the standard Tokio runtime
async fn main() {
    // Kafka producer configuration
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "rccp103-9e.iad3.prod.conviva.com:32300")
        .set("message.timeout.ms", "5000") // Kafka message timeout
        .set("linger.ms", "5")           // Wait 5ms for batching
        .set("batch.size", "65536")      // Batch size in bytes (64KB)
        .set("compression.type", "lz4")  // Enable LZ4 compression for better throughput
        .create()
        .expect("Failed to create Kafka producer");

    // Wrap the producer in an Arc for shared ownership
    let producer_arc = Arc::new(producer);

    // Set up the HTTP server
    let make_svc = make_service_fn(move |_| {
        let producer_clone = producer_arc.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| handle_request(req, producer_clone.clone())))
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
```

**Explanation:**
- `#[tokio::main]` sets up the default `tokio` runtime, which is compatible with both `hyper` and `rdkafka`.
- This removes the need for `tokio_uring`, resolving the runtime mismatch and the `spawn_local` panic.

---

#### **Option 2: Use `tokio_uring` for Kafka Only**
If you want to keep using `tokio_uring` for Kafka operations (e.g., for performance optimization), but still use `hyper` for the HTTP server, you’ll need to carefully separate the two runtimes.

Here’s how you can use `tokio` for the HTTP server and `tokio_uring` for Kafka operations:

1. Wrap the Kafka logic inside a `tokio_uring` runtime block, while keeping the HTTP server on the standard `tokio` runtime.

```rust
use tokio_uring::fs; // Example if `tokio_uring` is needed
use std::sync::Arc;
use tokio::task;

#[tokio::main] // Use the standard Tokio runtime for the HTTP server
async fn main() {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", "rccp103-9e.iad3.prod.conviva.com:32300")
        .set("message.timeout.ms", "5000")
        .set("linger.ms", "5")
        .set("batch.size", "65536")
        .set("compression.type", "lz4")
        .create()
        .expect("Failed to create Kafka producer");

    let producer_arc = Arc::new(producer);

    // Launch the HTTP server in a task
    let producer_clone = producer_arc.clone();
    let http_server = task::spawn(async move {
        let make_svc = make_service_fn(move |_| {
            let producer_clone = producer_clone.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req| handle_request(req, producer_clone.clone())))
            }
        });

        let addr = ([127, 0, 0, 1], 8080).into();
        let server = Server::bind(&addr).serve(make_svc);

        println!("Running server on http://127.0.0.1:8080");

        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    });

    // Use tokio-uring for Kafka operations if needed
    tokio_uring::start(async {
        // Example: Perform some I/O operations with tokio_uring (if required)
        println!("Using tokio_uring for I/O optimizations.");
    });

    // Await the HTTP server
    if let Err(e) = http_server.await {
        eprintln!("HTTP server error: {}", e);
    }
}
```

**Explanation:**
- The `tokio::main` runtime is used for the HTTP server.
- The `tokio_uring::start` runtime is used for specific Kafka operations or any I/O-heavy workloads.
- This allows you to isolate the two runtimes and prevent runtime conflicts.

---

#### **Option 3: Switch to a Fully `tokio_uring`-Compatible HTTP Library**
If you want to stick with `tokio_uring` for everything, including the HTTP server, consider switching to an HTTP library compatible with `tokio_uring`, such as **`glommio`** or another lightweight runtime.

However, this approach might require significant changes to your codebase and is less commonly used compared to the first two options.

---

### **Recommendation**
- Use **Option 1** (replace `tokio_uring::start` with the standard `tokio` runtime) unless you have a specific reason to use `tokio_uring` for performance.
- If you absolutely need `tokio_uring`, go with **Option 2** to isolate the runtimes.

Let me know if you need further clarification or help! 😊
*/

/*
To make the above code work purely using io_uring (via tokio-uring),
we must remove dependencies on libraries like hyper (which is built on the standard tokio runtime)
and implement an HTTP server manually using tokio-uring for asynchronous networking.
*/
