use crate::main_tcp_remote_kafka_base_producer::app_error::AppError;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{BaseProducer, BaseRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tokio_uring::net::{TcpListener, TcpStream};

#[path = "../../infra/app_error.rs"]
mod app_error;

// const KAFKA_TOPIC: &str = "aa-completion-flag-pt1m-fat";
// const KAFKA_BROKER: &str = "rccd101-6a.sjc2.dev.conviva.com:32511,rccd101-6b.sjc2.dev.conviva.com:32511,rccd101-6c.sjc2.dev.conviva.com:32511,rccd101-7a.sjc2.dev.conviva.com:32511";
// const KAFKA_TOPIC: &str = "sessionlet-completion-tlb2-aa-scalable-pt1m-dev";
// const KAFKA_BROKER: &str = "rccp103-9e.iad3.prod.conviva.com:32300";
const KAFKA_TOPIC: &str = "sessionlet-completion-tlb2-aa-scalable-pt1m-dev";
const KAFKA_BROKER: &str = "10.30.122.111:31210";


#[derive(Serialize, Deserialize, Debug)]
struct Payload {
    data: String,
}

pub(crate) fn main() -> std::io::Result<()> {
    tokio_uring::start(async {
        // Initialize the Kafka base producer
        let producer = Arc::new(
            ClientConfig::new()
                .set("bootstrap.servers", KAFKA_BROKER)
                .set("message.timeout.ms", "5000") // Timeout for Kafka acknowledgments
                .set("queue.buffering.max.kbytes", "10485760") // 10 GB
                .set("queue.buffering.max.messages", "5000000") // 5 million messages
                .set("acks", "1")
                .set("linger.ms", "50") // Wait up to 50ms for batching
                .set("batch.size", "262144") // Increase batch size to 256 KB
                .set("compression.type", "lz4")
                .set("debug", "all")
                .create::<BaseProducer>()
                .expect("Failed to create Kafka producer"),
        );

        // Bind the TCP listener to the desired address and port
        let listener = TcpListener::bind("127.0.0.1:8080".parse().unwrap())?;
        println!("Server is listening on 127.0.0.1:8080");

        let producer_for_polling = producer.clone();
        // Spawn a tokio-uring task to poll the producer
        tokio_uring::spawn(async move {
            loop {
                producer_for_polling.poll(Duration::from_millis(0));
                time::sleep(Duration::from_millis(100)).await;
            }
        });

        loop {
            // Accept new connections
            match listener.accept().await {
                Ok((stream, addr)) => {
                    // println!("Accepted connection from: {}", addr);
                    let producer = producer.clone();

                    // Spawn a new task to handle each client
                    tokio_uring::spawn(async move {
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
    })
}

/// Poll the producer for delivery reports in a loop.
// fn poll_producer(producer: Arc<BaseProducer>) {
//     loop {
//         producer.poll(Duration::from_millis(100));
//         std::thread::sleep(Duration::from_millis(100)); // Simulate periodic polling
//     }
// }

/// Handles a single client connection.
async fn handle_client(mut stream: TcpStream, producer: Arc<BaseProducer>) -> std::io::Result<()> {
    // Read data from the client
    let buf = vec![0; 1024];
    let buf = match read_from_stream(&mut stream, buf).await {
        Ok(buf) => buf,
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
            // Handle client disconnection gracefully
            // println!("Client disconnected gracefully.");
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

    // Publish to Kafka synchronously
    if let Err(err) = send_to_kafka(&producer, &payload.data) {
        eprintln!("Failed to send message to Kafka: {:?}", err);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            err.to_string(),
        ));
    }

    Ok(())
}

/// Reads data from the stream into a buffer.
async fn read_from_stream(stream: &mut TcpStream, buf: Vec<u8>) -> std::io::Result<Vec<u8>> {
    let (res, buf) = stream.read(buf).await;
    match res {
        Ok(read_bytes) if read_bytes > 0 => Ok(buf[..read_bytes].to_vec()),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "Client disconnected",
        )),
        Err(err) => Err(err),
    }
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

/// Parses the JSON payload from the buffer.
async fn parse_payload(buf: &[u8]) -> Result<Payload, serde_json::Error> {
    serde_json::from_slice(buf)
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

From the cluster rke-tlb-1.iad3.qe2.conviva.com and tlb2-aa-fat

"rccd101-6a.sjc2.dev.conviva.com:32511,rccd101-6b.sjc2.dev.conviva.com:32511,rccd101-6c.sjc2.dev.conviva.com:32511,rccd101-7a.sjc2.dev.conviva.com:32511"
"aa-completion-flag-pt1m-fat"



kafkacat -b rccd101-6a.sjc2.dev.conviva.com:32511,rccd101-6b.sjc2.dev.conviva.com:32511,rccd101-6c.sjc2.dev.conviva.com:32511,rccd101-7a.sjc2.dev.conviva.com:32511 -t aa-completion-flag-pt1m-fat -C -o end


From the cluster

kafkacat -b rccp103-9e.iad3.prod.conviva.com:32300 -t sessionlet-completion-tlb2-aa-scalable-pt1m-dev -C -o end


System Information:
  Total Memory: 3913.26 GB
  Used Memory: 700.77 GB
  Available Memory: 3212.49 GB
  Number of CPU Cores: 2
  CPU Brand: Intel(R) Xeon(R) CPU @ 2.20GHz
  CPU Frequency: 2199 MHz

Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated
io_uring_feedback_throughput_benchmark
                        time:   [374.28 ms 493.94 ms 605.21 ms]
                        change: [+98.556% +212.63% +446.53%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 1 outliers among 11 measurements (9.09%)
  1 (9.09%) high mild

Usually do for 300 seconds but did here for 100 seconds as the application stalls after some time

The throughput for 1000 API calls per iteration is:
Best Case (Fastest): ~2672 calls/sec
Average Case (Median): ~2024 calls/sec
Worst Case (Slowest): ~1652 calls/sec


For 200 seconds
Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated
io_uring_feedback_throughput_benchmark
                        time:   [357.60 ms 373.29 ms 401.39 ms]
                        change: [-34.379% -17.100% +4.6751%] (p = 0.20 > 0.05)
                        No change in performance detected.
Found 1 outliers among 11 measurements (9.09%)
  1 (9.09%) high mild

 The throughput for 1000 API calls per iteration is:

Best Case: ~2797 calls/sec
Median Case: ~2680 calls/sec
Worst Case: ~2490 calls/sec

It is getting bottle necked on kafka.
Because on local kafka, the app gets stuck. But once we increase the number of partitions on docker kafka
and increase the memory for kafka, the app runs smoothly. There is room for higher throughput.



For 100 seconds

io_uring_feedback_throughput_benchmark
                        time:   [356.16 ms 378.28 ms 409.12 ms]
                        change: [-40.660% -19.089% +24.878%] (p = 0.33 > 0.05)
                        No change in performance detected.
Found 1 outliers among 11 measurements (9.09%)
  1 (9.09%) high severe

kafkacat -b rccp103-9e.iad3.prod.conviva.com:32300 -t sessionlet-completion-tlb2-aa-scalable-pt1m-dev -C -o end

Throughput for 1000 requests:

Minimum time (356.16 ms):
2808.26 requests/sec

Median time (378.28 ms):
2643.86 requests/sec

Maximum time (409.12 ms):
2443.97 requests/sec



### Fresh benchmarking with file descriptors limit increased

Earlier it was 1024
Now ulimit -n 65536

100 seconds



kcat -b 10.30.122.111:31210 -t sessionlet-completion-tlb2-aa-scalable-pt1m-dev -C -o end

Benchmarking io_uring_feedback_throughput_benchmark: Warming up for
Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11
io_uring_feedback_throughput_benchmark
                        time:   [253.79 ms 254.81 ms 255.52 ms]
                        change: [-50.080% -49.551% -48.870%] (p = 0.00 < 0.05)
                        Performance has improved.

3939.5requests/second
3924.6requests/second
3913.9requests/second
*/



/*

Increase TCP buffers:

sudo sysctl -w net.core.rmem_max=26214400
sudo sysctl -w net.core.wmem_max=26214400

Increase the file descriptor limit:

ulimit -n 65536

4 core machine

kcat -b 10.30.122.111:31210 -t sessionlet-completion-tlb2-aa-scalable-pt1m-dev -C -o end
3000 requests per iteration

System Information:
  Total Memory: 15990.58 GB
  Used Memory: 671.49 GB
  Available Memory: 15319.09 GB
  Number of CPU Cores: 4
  CPU Brand: Intel(R) Xeon(R) CPU @ 2.20GHz
  CPU Frequency: 2199 MHz

#### ATTEMPT 1 ####

Benchmarking io_uring_feedback_throughput_benchmark: Collecting 11 samples in estimated 204.83 s (1452 iterations)


#### ATTEMPT 2 ####


*/
