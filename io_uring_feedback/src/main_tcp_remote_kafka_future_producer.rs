use crate::main_tcp_remote_kafka_future_producer::app_error::AppError;
use bytes::BytesMut;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_uring::net::{TcpListener, TcpStream};

#[path = "../../infra/app_error.rs"]
mod app_error;

const KAFKA_TOPIC: &str = "kafka-topic";
const KAFKA_BROKER: &str = "kafka-broker";
#[derive(Serialize, Deserialize, Debug)]
struct Payload {
    data: String,
}

pub(crate) fn main() -> std::io::Result<()> {
    tokio_uring::start(async {
        // Initialize the Kafka producer
        let producer = Arc::new(
            ClientConfig::new()
                .set(
                    "bootstrap.servers",
                    KAFKA_BROKER,
                )
                .set("message.timeout.ms", "5000") // Timeout for Kafka acknowledgments
                .set("linger.ms", "10") // Enable batching
                .set("batch.size", "262144") // Batch size in bytes (64 KB)
                .set("compression.type", "lz4")
                .create::<FutureProducer>()
                .expect("Failed to create Kafka producer"),
        );

        // Bind the TCP listener to the desired address and port
        let listener = TcpListener::bind("127.0.0.1:8080".parse().unwrap())?;
        println!("Server is listening on 127.0.0.1:8080");

        loop {
            // Accept new connections
            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("Accepted connection from: {}", addr);
                    let producer = producer.clone();

                    // Spawn a new task to handle each client
                    tokio_uring::spawn(async move {
                        if let Err(err) = handle_client(stream, producer).await {
                            eprintln!("Error handling client {}: {:?}", addr, err);
                        } else {
                            println!("Connection from {} handled successfully.", addr);
                        }
                    });
                }
                Err(err) => {
                    eprintln!("Failed to accept connection: {:?}", err);
                }
            }
        }
    })
}

/// Handles a single client connection.
async fn handle_client(
    mut stream: TcpStream,
    producer: Arc<FutureProducer>,
) -> std::io::Result<()> {
    // Read data from the client
    let buf = vec![0; 1024];
    let buf = match read_from_stream(&mut stream, buf).await {
        Ok(buf) => buf,
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
            // Handle client disconnection gracefully
            println!("Client disconnected gracefully.");
            return Ok(()); // No further processing required
        }
        Err(err) => {
            eprintln!("Failed to read from stream: {:?}", err);
            write_to_stream(&mut stream, b"Failed to read data").await?;
            return Err(err);
        }
    };

    // Parse the HTTP request and extract the body
    let body = match parse_http_request(&buf) {
        Ok(body) => body,
        Err(err) => {
            eprintln!("Failed to parse HTTP request: {:?}", err);
            write_to_stream(&mut stream, b"Invalid HTTP request format").await?;
            return Ok(());
        }
    };

    // Parse the JSON payload from the HTTP body
    let payload: Payload = match parse_payload(&body).await {
        Ok(payload) => payload,
        Err(err) => {
            eprintln!("Invalid payload format: {:?}", err);
            write_to_stream(&mut stream, b"Invalid payload format").await?;
            return Ok(()); // Send a response but do not terminate the server
        }
    };

    // Immediately send an acknowledgment response to the client
    if let Err(err) = write_to_stream(&mut stream, b"Payload received").await {
        eprintln!("Failed to send acknowledgment to client: {:?}", err);
        return Err(err);
    }

    // Publish to Kafka asynchronously after sending the acknowledgment
    tokio_uring::spawn(async move {
        if let Err(err) = send_to_kafka(&producer, &payload.data).await {
            eprintln!("Failed to send to Kafka: {:?}", err);
        }
    });

    Ok(())
}

/// Reads data from the stream into a buffer.
async fn read_from_stream(stream: &mut TcpStream, buf: Vec<u8>) -> std::io::Result<Vec<u8>> {
    let (res, buf) = stream.read(buf).await;
    match res {
        Ok(read_bytes) if read_bytes > 0 => Ok(buf[..read_bytes].to_vec()),
        Ok(_) => {
            // Client closed the connection (EOF), return a specific error for graceful handling
            Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Client disconnected",
            ))
        }
        Err(err) => Err(err),
    }
}

/// Parses the HTTP request and extracts the body.
fn parse_http_request(buf: &[u8]) -> Result<&[u8], &'static str> {
    let request = String::from_utf8_lossy(buf);

    // Find the body separator
    if let Some(body_start) = request.find("\r\n\r\n") {
        let body = &buf[body_start + 4..];
        Ok(body)
    } else {
        Err("Invalid HTTP request format: Missing header-body separator")
    }
}

/// Parses the JSON payload from the buffer.
async fn parse_payload(buf: &[u8]) -> Result<Payload, serde_json::Error> {
    serde_json::from_slice(buf)
}

/// Sends the current payload to Kafka using the producer.
async fn send_to_kafka(producer: &FutureProducer, current_payload: &str) -> Result<(), AppError> {
    let record = FutureRecord::to(KAFKA_TOPIC)
        .payload(current_payload)
        .key("key");
    producer.send(record, rdkafka::util::Timeout::Never).await?;
    Ok(())
}

/// Writes a response to the stream.
async fn write_to_stream(stream: &mut TcpStream, response: &[u8]) -> std::io::Result<()> {
    let http_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        response.len(),
        String::from_utf8_lossy(response)
    );

    let write_op = stream.write(http_response.into_bytes());
    let (res, _) = write_op.submit().await;

    match res {
        Ok(_) => Ok(()),
        Err(err) => {
            eprintln!("Failed to write to stream: {:?}", err);
            Err(err)
        }
    }
}

/*
Benchmark results

Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated
io_uring_feedback_throughput_benchmark
                        time:   [419.99 ms 506.34 ms 621.30 ms]
                        change: [-99.219% -98.848% -97.885%] (p = 0.00 < 0.05)
                        Performance has improved.

1000 requests in one iteration


- **Throughput Range:** `1609.0 - 2381.0 requests/sec`
- **Average Throughput:** `1974.9 requests/sec` (based on the mid value)
*/


/*
1000 req
200 seconds

Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated 205.90 s (462 iterations)
*/