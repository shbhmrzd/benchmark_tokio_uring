# Async Benchmark

## Rust HTTP-to-Kafka Benchmark Project

This project benchmarks two different approaches to implementing an HTTP-to-Kafka API server in Rust. The server receives JSON payloads via HTTP, processes them, and sends the data to a Kafka topic. The two implementations compare the performance of using:

1. **`tokio-uring` with raw TCP streams** (low-level, high-performance I/O based on `io_uring`).
2. **`hyper` with the `tokio` runtime** (high-level HTTP framework).

## Project Structure

- `tokio_server`: A server using the Tokio runtime.
- `io_uring_server`: A server using the io_uring interface.
- `benches/benchmark.rs`: Contains the benchmarking code.
- `Cargo.toml`: Project dependencies and configuration.

The project contains the following implementations:

### 1. `tokio-uring` Implementation
- Uses **`tokio-uring`** for asynchronous I/O with raw TCP streams.
- Manually parses HTTP requests and extracts the JSON payload from the body.
- Sends the extracted JSON payload to Kafka using the `rdkafka` library.
- Designed for maximum performance and efficiency by directly interfacing with the `io_uring` system call.

### 2. `hyper` Implementation
- Uses **`hyper`**, a high-level HTTP framework built on `tokio`.
- Automatically parses HTTP requests and provides abstractions for handling JSON payloads.
- Sends the JSON payload to Kafka using the `rdkafka` library.
- Easier to use but comes with higher overhead due to HTTP abstractions.


## Dependencies

- `criterion`: For benchmarking.
- `hyper`: HTTP client library.
- `tokio`: Asynchronous runtime.


## How It Works

### Workflow
1. The server listens for incoming requests on a specific port (`8080` for `hyper`, `8081` for `tokio-uring`).
2. Each client sends a POST request with a JSON payload (e.g., `{"data": "test_payload"}`).
3. The server processes the payload and forwards it to a Kafka topic (`test_topic_io_uring` for `tokio-uring`, `test_topic_tokio` for `hyper`).
4. The server responds with `Payload received` after processing the request.

### Kafka Integration
The Kafka setup uses the `rdkafka` library to:
- Produce messages to the Kafka topic.
- Retry automatically in case of transient errors.


## Running the Benchmarks

To run the benchmarks, use the following command:

```sh
cargo bench
```