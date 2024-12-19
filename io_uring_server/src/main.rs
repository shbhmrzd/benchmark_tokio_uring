use crate::app_error::AppError;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use testcontainers::clients::Cli;
use tokio_uring::net::{TcpListener, TcpStream};

#[path = "../../infra/app_error.rs"]
mod app_error;
#[path = "../../infra/kafka.rs"]
mod kafka;

use crate::kafka::set_up_kafka;

#[derive(Serialize, Deserialize, Debug)]
struct Payload {
    data: String,
}

fn main() -> std::io::Result<()> {
    tokio_uring::start(async {
        // println!("Starting server - 1");

        let docker = Cli::default();

        let (bootstrap_servers, _kafka_container, _zk_container) = set_up_kafka(&docker);

        // println!("Kafka setup complete - 2");

        let producer = Arc::new(
            ClientConfig::new()
                .set("bootstrap.servers", &bootstrap_servers)
                .create::<FutureProducer>()
                .expect("Failed to create Kafka producer"),
        );

        // println!("Kafka producer created - 3");

        let last_payload = Arc::new(Mutex::new(None::<String>));

        let listener = TcpListener::bind("127.0.0.1:8081".parse().unwrap())?;
        // println!("Server is listening on 127.0.0.1:8081 - 4");
        println!("Bootstrap servers: {}", bootstrap_servers);

        loop {
            // println!("Waiting for a new connection - 5");
            match listener.accept().await {
                Ok((stream, _)) => {
                    // println!("Accepted a connection - 6");
                    let producer = producer.clone();
                    let last_payload = last_payload.clone();

                    tokio_uring::spawn(async move {
                        // println!("Handling client in new task - 7");
                        if let Err(err) = handle_client(stream, producer, last_payload).await {
                            eprintln!("Error handling client: {:?}", err);
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

async fn handle_client(
    mut stream: TcpStream,
    producer: Arc<FutureProducer>,
    last_payload: Arc<Mutex<Option<String>>>,
) -> std::io::Result<()> {
    // println!("Inside handle_client - 8");

    // Read data from the client
    let buf = vec![0; 1024];
    let buf = match read_from_stream(&mut stream, buf).await {
        Ok(buf) => buf,
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
        Ok(payload) => {
            // println!("Parsed payload successfully: {:?}", payload);
            payload
        }
        Err(err) => {
            eprintln!("Invalid payload format: {:?}", err);
            write_to_stream(&mut stream, b"Invalid payload format").await?;
            return Ok(()); // Send a response but do not terminate the server
        }
    };

    // Handle Kafka message
    if let Err(err) = send_to_kafka(&producer, &last_payload, &payload.data).await {
        eprintln!("Failed to send to Kafka: {:?}", err);
        write_to_stream(&mut stream, b"Failed to send data to Kafka").await?;
        return Ok(());
    }

    // Update the shared payload
    update_last_payload(&last_payload, &payload.data);

    // Send a success response to the client
    write_to_stream(&mut stream, b"Payload received").await
}

/// Reads data from the stream into a buffer.
async fn read_from_stream(stream: &mut TcpStream, buf: Vec<u8>) -> std::io::Result<Vec<u8>> {
    let (res, buf) = stream.read(buf).await;
    match res {
        Ok(read_bytes) if read_bytes > 0 => {
            // println!(
            //     "Read {} bytes from client: {}",
            //     read_bytes,
            //     String::from_utf8_lossy(&buf[..read_bytes])
            // );
            Ok(buf[..read_bytes].to_vec())
        }
        Ok(_) => {
            eprintln!("Client closed the connection unexpectedly.");
            Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Client disconnected",
            ))
        }
        Err(err) => {
            eprintln!("Error reading from stream: {:?}", err);
            Err(err)
        }
    }
}

/// Parses the HTTP request and extracts the body.
fn parse_http_request(buf: &[u8]) -> Result<&[u8], &'static str> {
    let request = String::from_utf8_lossy(buf);
    // println!("Full HTTP request: {}", request);

    // Find the body separator
    if let Some(body_start) = request.find("\r\n\r\n") {
        let body = &buf[body_start + 4..];
        // println!("Extracted HTTP body: {}", String::from_utf8_lossy(body));
        Ok(body)
    } else {
        Err("Invalid HTTP request format: Missing header-body separator")
    }
}

/// Parses the JSON payload from the buffer.
async fn parse_payload(buf: &[u8]) -> Result<Payload, serde_json::Error> {
    // println!(
    //     "Attempting to parse JSON payload: {}",
    //     String::from_utf8_lossy(buf)
    // );
    let parsed_payload = serde_json::from_slice(buf)?;
    Ok(parsed_payload)
}

/// Sends the payload to Kafka using the producer.
async fn send_to_kafka(
    producer: &FutureProducer,
    last_payload: &Arc<Mutex<Option<String>>>,
    current_payload: &str,
) -> Result<(), AppError> {
    // println!("Preparing to send data to Kafka: {}", current_payload);

    let previous_payload = last_payload.lock().unwrap().clone();
    if let Some(prev) = previous_payload {
        // println!("Sending previous payload to Kafka: {}", prev);
        let record = FutureRecord::to("test_topic_io_uring")
            .payload(&prev)
            .key("key");
        producer.send(record, rdkafka::util::Timeout::Never).await?;
    }

    // println!("Sending current payload to Kafka: {}", current_payload);
    let record = FutureRecord::to("test_topic_io_uring")
        .payload(current_payload)
        .key("key");
    producer.send(record, rdkafka::util::Timeout::Never).await?;
    Ok(())
}

/// Updates the shared payload with the current payload.
fn update_last_payload(last_payload: &Arc<Mutex<Option<String>>>, payload: &str) {
    let mut last_payload = last_payload.lock().unwrap();
    *last_payload = Some(payload.to_string());
    // println!("Updated last payload in memory: {}", payload);
}

/// Writes a response to the stream.
async fn write_to_stream(stream: &mut TcpStream, response: &[u8]) -> std::io::Result<()> {
    let http_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        response.len(),
        String::from_utf8_lossy(response)
    );

    // println!("Sending response to client: {}", String::from_utf8_lossy(response));

    let mut total_written = 0;
    let owned_response = http_response.into_bytes();

    while total_written < owned_response.len() {
        let buf = &owned_response[total_written..];

        let write_op = stream.write(buf.to_vec());
        let (res, _buf) = write_op.submit().await;

        match res {
            Ok(written) => {
                total_written += written;
            }
            Err(err) => {
                eprintln!("Failed to write to stream: {:?}", err);
                return Err(err);
            }
        }
    }

    Ok(())
}
