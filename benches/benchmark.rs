use criterion::Criterion as CriterionConfig; // Alias Criterion configuration to customize it
use criterion::{criterion_group, criterion_main, Criterion}; // Import Criterion for benchmarking
use futures::future::join_all; // Used to execute concurrent tasks
use hyper::Body; // HTTP request body handling
use hyper::Client; // HTTP client for sending requests
use hyper::Request; // Used to build HTTP requests
use sysinfo::{CpuExt, System, SystemExt};
use tokio::runtime::Runtime; // Tokio runtime for async execution // Import sysinfo to get system details

// Number of concurrent requests to stress test the server
const CONCURRENT_REQUESTS: usize = 50;

// Total number of requests for throughput benchmarks
const TOTAL_REQUESTS: usize = 3000;

// Adjust Criterion configuration to avoid warnings and control benchmark behavior
fn configure_criterion() -> CriterionConfig {
    CriterionConfig::default()
        .measurement_time(std::time::Duration::from_secs(300))
        .sample_size(11) // Reduce the number of samples to 50 for faster benchmarking
        .warm_up_time(std::time::Duration::from_secs(5)) // Add a warm-up time of 5 seconds to stabilize the server
}

// Function to log system information
fn log_system_info() {
    let mut system = System::new_all(); // Initialize the system information collector
    system.refresh_all(); // Refresh to get up-to-date information

    // Print system information
    println!("System Information:");
    println!(
        "  Total Memory: {:.2} GB",
        system.total_memory() as f64 / 1_048_576.0
    );
    println!(
        "  Used Memory: {:.2} GB",
        system.used_memory() as f64 / 1_048_576.0
    );
    println!(
        "  Available Memory: {:.2} GB",
        system.available_memory() as f64 / 1_048_576.0
    );
    println!("  Number of CPU Cores: {}", system.cpus().len());
    println!("  CPU Brand: {}", system.global_cpu_info().brand());
    println!(
        "  CPU Frequency: {} MHz",
        system.global_cpu_info().frequency()
    );
    println!();
}

// Benchmark for single-request performance of the Tokio server
fn tokio_benchmark(c: &mut Criterion) {
    // Log system info before running benchmarks
    log_system_info();
    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Define the benchmark
    c.bench_function("tokio_server", |b| {
        b.iter(|| {
            rt.block_on(async {
                let client = Client::new(); // Create a new Hyper client
                let req = Request::post("http://127.0.0.1:8080/payload") // Build an HTTP POST request
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"data":"test_tokio"}"#)) // Send a JSON payload
                    .unwrap();

                let _ = client.request(req).await; // Execute the request
            });
        });
    });
}

// Benchmark for single-request performance of the io_uring server
fn io_uring_benchmark(c: &mut Criterion) {
    log_system_info();
    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Define the benchmark
    c.bench_function("io_uring_server", |b| {
        b.iter(|| {
            rt.block_on(async {
                let client = Client::new(); // Create a new Hyper client
                let req = Request::post("http://127.0.0.1:8081/payload") // Build an HTTP POST request
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"data":"test_io_uring"}"#)) // Send a JSON payload
                    .unwrap();

                let _ = client.request(req).await; // Execute the request
            });
        });
    });
}

// Benchmark for concurrent performance of the Tokio server
fn tokio_concurrent_benchmark(c: &mut Criterion) {
    log_system_info();
    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Define the benchmark
    c.bench_function("tokio_server_concurrent", |b| {
        b.iter(|| {
            rt.block_on(async {
                let client = Client::new(); // Create a new Hyper client
                let tasks = (0..CONCURRENT_REQUESTS).map(|_| {
                    let client = client.clone(); // Clone the client for each task
                    async move {
                        let req = Request::post("http://127.0.0.1:8080/payload") // Build an HTTP POST request
                            .header("content-type", "application/json")
                            .body(Body::from(r#"{"data":"test_tokio"}"#)) // Send a JSON payload
                            .unwrap();

                        let _ = client.request(req).await; // Execute the request
                    }
                });

                // Execute all tasks concurrently
                join_all(tasks).await;
            });
        });
    });
}

// Benchmark for concurrent performance of the io_uring server
fn io_uring_concurrent_benchmark(c: &mut Criterion) {
    log_system_info();
    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Define the benchmark
    c.bench_function("io_uring_server_concurrent", |b| {
        b.iter(|| {
            rt.block_on(async {
                let client = Client::new(); // Create a new Hyper client
                let tasks = (0..CONCURRENT_REQUESTS).map(|_| {
                    let client = client.clone(); // Clone the client for each task
                    async move {
                        let req = Request::post("http://127.0.0.1:8081/payload") // Build an HTTP POST request
                            .header("content-type", "application/json")
                            .body(Body::from(r#"{"data":"test_io_uring"}"#)) // Send a JSON payload
                            .unwrap();

                        let _ = client.request(req).await; // Execute the request
                    }
                });

                // Execute all tasks concurrently
                join_all(tasks).await;
            });
        });
    });
}

// Throughput benchmark for the Tokio server
fn tokio_throughput_benchmark(c: &mut Criterion) {
    log_system_info();
    let rt = Runtime::new().unwrap();

    c.bench_function("tokio_server_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                let client = Client::new(); // Create a new Hyper client
                let tasks = (0..TOTAL_REQUESTS).map(|_| {
                    let client = client.clone();
                    async move {
                        let req = Request::post("http://127.0.0.1:8080/payload") // Build an HTTP POST request
                            .header("content-type", "application/json")
                            .body(Body::from(r#"{"data":"test_tokio"}"#)) // Send a JSON payload
                            .unwrap();

                        let _ = client.request(req).await; // Execute the request
                    }
                });

                // Execute all tasks and measure throughput
                join_all(tasks).await;
            });
        });
    });
}

// Throughput benchmark for the io_uring server
fn io_uring_throughput_benchmark(c: &mut Criterion) {
    log_system_info();
    let rt = Runtime::new().unwrap();

    c.bench_function("io_uring_server_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                let client = Client::new(); // Create a new Hyper client
                let tasks = (0..TOTAL_REQUESTS).map(|_| {
                    let client = client.clone();
                    async move {
                        let req = Request::post("http://127.0.0.1:8081/payload") // Build an HTTP POST request
                            .header("content-type", "application/json")
                            .body(Body::from(r#"{"data":"test_io_uring"}"#)) // Send a JSON payload
                            .unwrap();

                        let _ = client.request(req).await; // Execute the request
                    }
                });

                // Execute all tasks and measure throughput
                join_all(tasks).await;
            });
        });
    });
}

// Throughput benchmark for the io_uring_feedback server
fn io_uring_feedback_throughput_benchmark(c: &mut Criterion) {
    log_system_info();
    let rt = Runtime::new().unwrap();

    c.bench_function("io_uring_feedback_throughput_benchmark", |b| {
        b.iter(|| {
            rt.block_on(async {
                let client = Client::new(); // Create a new Hyper client
                let tasks = (0..TOTAL_REQUESTS).map(|_| {
                    let client = client.clone();
                    async move {
                        let req = Request::post("http://127.0.0.1:8080/") // Build an HTTP POST request
                            .header("content-type", "application/json")
                            .body(Body::from(r#"{"data":"test_io_uring_feedback"}"#)) // Send a JSON payload
                            .unwrap();

                        let _ = client.request(req).await; // Execute the request
                    }
                });

                // Execute all tasks and measure throughput
                join_all(tasks).await;
            });
        });
    });
}

// Define the benchmark group with all benchmarks
criterion_group! {
    name = benches; // Name of the benchmark group
    config = configure_criterion(); // Use the custom Criterion configuration
    // targets = tokio_benchmark, io_uring_benchmark, tokio_concurrent_benchmark, io_uring_concurrent_benchmark, tokio_throughput_benchmark, io_uring_throughput_benchmark // Include throughput benchmarks
    targets = io_uring_feedback_throughput_benchmark // Include throughput benchmarks
}

// Define the main function to execute the benchmarks
criterion_main!(benches);
