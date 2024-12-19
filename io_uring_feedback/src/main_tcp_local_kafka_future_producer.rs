use crate::main_tcp_local_kafka_future_producer::app_error::AppError;
use crate::main_tcp_local_kafka_future_producer::kafka::set_up_kafka;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use testcontainers::clients::Cli;
use tokio_uring::net::{TcpListener, TcpStream};

#[path = "../../infra/app_error.rs"]
mod app_error;

#[path = "../../infra/kafka.rs"]
mod kafka;

#[derive(Serialize, Deserialize, Debug)]
struct Payload {
    data: String,
}

fn main() -> std::io::Result<()> {
    tokio_uring::start(async {
        let docker = Cli::default();

        // Set up Kafka using testcontainers
        let (bootstrap_servers, _kafka_container, _zk_container) = set_up_kafka(&docker);
        println!("Bootstrap servers: {}", bootstrap_servers);

        // Initialize the Kafka producer
        let producer = Arc::new(
            ClientConfig::new()
                .set("bootstrap.servers", bootstrap_servers.as_str())
                .set("message.timeout.ms", "5000") // Timeout for Kafka acknowledgments
                .set("linger.ms", "5") // Enable batching
                .set("batch.size", "65536") // Batch size in bytes (64 KB)
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
    let record = FutureRecord::to("sessionlet-completion-tlb2-aa-scalable-pt1m-dev")
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
kafkacat -b 127.0.0.1:32867 -t sessionlet-completion-tlb2-aa-scalable-pt1m-dev -C -o end

Benchmark results

Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated
io_uring_feedback_throughput_benchmark
                        time:   [555.56 ms 721.98 ms 826.74 ms]
                        change: [+43.881% +69.221% +100.93%] (p = 0.00 < 0.05)
                        Performance has regressed.

200 seconds
1000 requests in one iteration


Based on the benchmark times, the throughput is:

Best Case (Minimum Time): ~1800 calls/sec
Median Case: ~1385 calls/sec
Worst Case (Maximum Time): ~1210 calls/sec

*/
