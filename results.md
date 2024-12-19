# Benchmark Results

## Overview

This document summarizes the benchmark results for evaluating the performance of **Tokio Server** and **io_uring Server**.
Both single-request and concurrent-request scenarios were tested using Criterion.

### Test Environment

- **Benchmark Tool**: Criterion
- **Concurrent Requests**: 50
- **Criterion Configuration**:
    - `measurement_time`: 90 seconds
    - `sample_size`: 50
    - `warm_up_time`: 5 seconds
- **Servers**:
    - **Tokio Server**: `http://127.0.0.1:8080`
    - **io_uring Server**: `http://127.0.0.1:8081`

## Results Table

| **Benchmark**                   | **Average Time (ms)** | **Best Time (ms)** | **Worst Time (ms)** | **Notes**                             |
|----------------------------------|-----------------------|--------------------|---------------------|---------------------------------------|
| `tokio_server`                  | 7.94–8.00            | 7.94               | 8.00                | Single-request latency                |
| `io_uring_server`               | 14.51–14.59          | 14.51              | 14.59               | Single-request latency                |
| `tokio_server_concurrent`       | 356.74–370.04        | 356.74             | 370.04              | 50 concurrent requests, high variance |
| `io_uring_server_concurrent`    | 22.56–22.75          | 22.56              | 22.75               | 50 concurrent requests, stable performance |


## Key Findings

1. **Single-Request Performance**:
    - **Tokio Server** performed better in single-request scenarios with an average latency of ~8ms.
    - **io_uring Server** had higher single-request latency, averaging ~14.5ms.

2. **Concurrent-Request Performance**:
    - **io_uring Server** handled 50 concurrent requests significantly faster, with an average latency of ~22.7ms.
    - **Tokio Server** struggled with concurrent requests, averaging ~362.6ms.

3. **Stability**:
    - Both servers exhibited minimal outliers in single-request scenarios.
    - In concurrent-request scenarios:
        - **Tokio Server** had 6% severe outliers, indicating high variability under load.
        - **io_uring Server** was more stable with fewer mild outliers.


## Observations

### **Tokio Server**
- Performs well for single requests.
- Experiences significant performance degradation with high concurrency, indicating possible bottlenecks in handling concurrent tasks.

### **io_uring Server**
- Higher latency for single requests compared to Tokio.
- Excels in concurrent scenarios, demonstrating significantly better performance under load.


## Benchmark Execution

### Steps to Reproduce

1. **Start the Servers**:
    - Start the `tokio_server`:
      ```bash
      cargo run -p tokio_server
      ```
    - Start the `io_uring_server`:
      ```bash
      cargo run -p io_uring_server
      ```

2. **Run the Benchmarks**:
    - Execute the benchmarks using Criterion:
      ```bash
      cargo bench
      ```
    

# Throughput Comparison

## System Information
| **Metric**               | **Value**                          |
|--------------------------|------------------------------------|
| **Total Memory**         | 3913.26 GB                        |
| **Used Memory**          | 1815.66 GB                        |
| **Available Memory**     | 2097.60 GB                        |
| **Number of CPU Cores**  | 2                                 |
| **CPU Brand**            | Intel(R) Xeon(R) CPU @ 2.20GHz    |
| **CPU Frequency**        | 2199 MHz                          |

---

## Benchmark Configuration
| **Metric**               | **Value**                          |
|--------------------------|------------------------------------|
| **Requests per Iteration** | 1000 (from `TOTAL_REQUESTS`)      |
| **Number of Samples**    | 10 (from `sample_size`)            |
| **Measurement Time**     | 300 seconds (from `measurement_time`) |

---

## Benchmark Results

### Tokio Server Throughput
| **Metric**                   | **Value**                                |
|------------------------------|------------------------------------------|
| **Time per Iteration**       | ~7.4586 seconds                         |
| **Requests per Iteration**   | 1000                                    |
| **Throughput**               | \( \frac{1000 \text{ requests}}{7.4586 \text{ s}} \) ≈ **134 requests/second** |
| **Iterations per Sample**    | ~1 (Criterion adjusted for the long iteration time) |
| **Total Requests per Sample**| 1000                                    |
| **Total Requests (All Samples)** | \( 1000 \times 10 = 10,000 \)       |

---

### io_uring Server Throughput
| **Metric**                   | **Value**                                |
|------------------------------|------------------------------------------|
| **Time per Iteration**       | ~657.31 milliseconds (median)           |
| **Requests per Iteration**   | 1000                                    |
| **Throughput**               | \( \frac{1000 \text{ requests}}{0.65731 \text{ s}} \) ≈ **1521 requests/second** |
| **Iterations per Sample**    | ~990                                    |
| **Total Requests per Sample**| \( 1000 \times 990 = 990,000 \)         |
| **Total Requests (All Samples)** | \( 990,000 \times 10 = 9,900,000 \) |

---

## Summary Table

| **Benchmark**                | **Requests/Iteration** | **Time/Iteration** | **Throughput (Requests/s)** | **Total Requests** |
|------------------------------|------------------------|--------------------|----------------------------|--------------------|
| **Tokio Server Throughput**  | 1000                  | ~7.4586 s          | ~134                       | 10,000            |
| **io_uring Server Throughput**| 1000                 | ~657.31 ms         | ~1521                      | 9,900,000         |

---

## Key Observations
1. **io_uring Outperforms Tokio**:
    - The io_uring-based server demonstrates significantly higher throughput at ~**1521 requests/second** compared to ~**134 requests/second** for the Tokio-based server.
    - This is expected, as io_uring is designed for highly efficient, low-level I/O operations.

2. **Runtime Efficiency**:
    - io_uring's **time per iteration (~657 ms)** is much shorter than Tokio's **time per iteration (~7.458 seconds)**, making it better suited for workloads requiring high throughput and low latency.

3. **CPU Usage**:
    - With only 2 CPU cores available, io_uring is more effective at leveraging system resources compared to Tokio's relatively heavier runtime overhead.

4. **Total Requests**:
    - Over 10 samples, the io_uring server processed **~9.9 million requests** compared to the Tokio server's **10,000 requests**.
