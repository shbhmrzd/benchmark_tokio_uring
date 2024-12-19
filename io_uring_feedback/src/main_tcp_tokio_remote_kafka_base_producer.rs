use crate::main_tcp_tokio_remote_kafka_base_producer::app_error::AppError;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{BaseProducer, BaseRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time;

#[path = "../../infra/app_error.rs"]
mod app_error;

const KAFKA_TOPIC: &str = "kafka-topic";
const KAFKA_BROKER: &str = "kafka-broker";

#[derive(Serialize, Deserialize, Debug)]
struct Payload {
    data: String,
}

#[tokio::main]
pub(crate) async fn main() -> std::io::Result<()> {
    // Initialize the Kafka base producer
    let producer = Arc::new(
        ClientConfig::new()
            .set("bootstrap.servers", KAFKA_BROKER)
            .set("message.timeout.ms", "5000") // Timeout for Kafka acknowledgments
            .set("queue.buffering.max.messages", "1000000") // 1 million messages
            .set("queue.buffering.max.kbytes", "5485760") // 5 GB buffer
            .set("linger.ms", "50") // Wait up to 50ms for batching
            .set("batch.size", "262144") // Increase batch size to 256 KB
            .set("compression.type", "lz4")
            .set("debug", "all")
            .create::<BaseProducer>()
            .expect("Failed to create Kafka producer"),
    );

    // Start a task to poll the Kafka producer periodically
    let producer_for_polling = producer.clone();
    tokio::spawn(async move {
        loop {
            producer_for_polling.poll(Duration::from_millis(0));
            time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Bind the TCP listener to the desired address and port
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server is listening on 127.0.0.1:8080");

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                // println!("Accepted connection from: {}", addr);
                let producer = producer.clone();

                // Spawn a new tokio task to handle each client
                tokio::spawn(async move {
                    if let Err(err) = handle_client(stream, producer).await {
                        eprintln!("Error handling client {}: {:?}", addr, err);
                    } else {
                        // println!("Connection from {} handled successfully.", addr);
                    }
                });
            }
            Err(err) => {
                eprintln!("Failed to accept connection: {:?}", err);
            }
        }
    }
}

/// Handles a single client connection.
async fn handle_client(mut stream: TcpStream, producer: Arc<BaseProducer>) -> std::io::Result<()> {
    let mut buf = vec![0; 1024];

    loop {
        // Read data from the client
        let n = match stream.read(&mut buf).await {
            Ok(0) => {
                // println!("Client disconnected gracefully.");
                return Ok(());
            }
            Ok(n) => n,
            Err(e) => {
                eprintln!("Failed to read from stream: {:?}", e);
                return Err(e);
            }
        };

        // Parse the HTTP request and extract the body
        let body = match parse_http_request(&buf[..n]) {
            Ok(body) => body,
            Err(err) => {
                eprintln!("Failed to parse HTTP request: {}", err);
                write_to_stream(&mut stream, b"Invalid HTTP request format").await?;
                continue;
            }
        };

        // Parse the JSON payload
        let payload: Payload = match serde_json::from_slice(body) {
            Ok(payload) => payload,
            Err(err) => {
                eprintln!("Invalid payload format: {:?}", err);
                write_to_stream(&mut stream, b"Invalid payload format").await?;
                continue;
            }
        };

        // println!("Received payload: {:?}", payload);

        // Send acknowledgment to the client
        write_to_stream(&mut stream, b"Payload received").await?;

        // Publish to Kafka synchronously
        if let Err(err) = send_to_kafka(&producer, &payload.data) {
            eprintln!("Failed to send message to Kafka: {:?}", err);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                err.to_string(),
            ));
        }
    }
}

/// Writes a response to the stream.
async fn write_to_stream(stream: &mut TcpStream, response: &[u8]) -> std::io::Result<()> {
    let http_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        response.len(),
        String::from_utf8_lossy(response)
    );

    stream.write_all(http_response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

/// Parses the HTTP request and extracts the body.
fn parse_http_request(buf: &[u8]) -> Result<&[u8], &'static str> {
    let request = String::from_utf8_lossy(buf);

    if let Some(body_start) = request.find("\r\n\r\n") {
        let body = &buf[body_start + 4..];
        Ok(body)
    } else {
        Err("Invalid HTTP request format: Missing header-body separator")
    }
}

/// Sends the current payload to Kafka using the producer.
fn send_to_kafka(producer: &BaseProducer, current_payload: &str) -> Result<(), AppError> {
    let record = BaseRecord::to(KAFKA_TOPIC)
        .payload(current_payload)
        .key("key");

    match producer.send(record) {
        Ok(_) => Ok(()),
        Err((err, _)) => {
            eprintln!("Failed to send message to Kafka: {:?}", err);
            Err(AppError::KafkaError(err))
        }
    }
}



/*

ulimit -n 65536

System Information:
  Total Memory: 3913.26 GB
  Used Memory: 600.44 GB
  Available Memory: 3312.82 GB
  Number of CPU Cores: 2
  CPU Brand: Intel(R) Xeon(R) CPU @ 2.20GHz
  CPU Frequency: 2199 MHz

Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated 106.41 s (462 iterations)

100 seconds
1000 requests per iteration


Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimate
io_uring_feedback_throughput_benchmark
                        time:   [214.73 ms 219.19 ms 224.24 ms]
                        change: [-28.013% +24.303% +185.86%] (p = 0.50 > 0.05)
                        No change in performance detected.


Lower bound = 4459.9ops/sec
Upper bound = 4656.2ops/sec
*/



/*
4 core machine

kcat -b kafka-broker -t kafka-topic -C -o end
3000 requests per iteration

System Information:
  Total Memory: 15990.58 GB
  Used Memory: 671.49 GB
  Available Memory: 15319.09 GB
  Number of CPU Cores: 4
  CPU Brand: Intel(R) Xeon(R) CPU @ 2.20GHz
  CPU Frequency: 2199 MHz

#### ATTEMPT 1 ####
Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated 305.97 s (792 iterations)


io_uring_feedback_throughput_benchmark
                        time:   [348.25 ms 350.32 ms 354.24 ms]
                        change: [+167.12% +175.12% +184.45%] (p = 0.00 < 0.05)
                        Performance has regressed.

3000 requests per iteration
Lower = 8468.71req/s
Median = 8564.45req/s
Upper = 8616.41req/s



#### ATTEMPT 2 ####
Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated 303.15 s (792 iterations)

Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated
io_uring_feedback_throughput_benchmark
                        time:   [345.90 ms 346.86 ms 348.84 ms]
                        change: [-4.8349% -2.0961% +0.8354%] (p = 0.19 > 0.05)
                        No change in performance detected.
Found 1 outliers among 11 measurements (9.09%)
  1 (9.09%) high severe

3000 requests per iteration
Lower = 8602.08req/s
Median = 8650.84req/s
Upper = 8675.46req/s

*/