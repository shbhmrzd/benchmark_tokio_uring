# Performance Analysis
## Tokio vs Tokio-Uring for Kafka Event Streaming

**Objective**: Maximize Webserver Throughput with Kafka Integration

---

# Objective

- **Goal**:
    - Build a performant Rust webserver for high-throughput HTTP requests.
    - Send Kafka events for each incoming request payload.
- **Metrics to Compare**:
    - Max requests/second (throughput).
    - Stability under high load.
    - Observations of bottlenecks.

---

# System Setup

- **Hardware Specs**:
    - **CPU**: 2-core Intel Xeon @ 2.20GHz.
    - **Memory**: 3913.26 GB (573.5 GB used).
    - **OS**: Linux-based system. 
    ```shell
    root@shubham-raizada-5-10:~/shubham# uname -a
    # Linux shubham-raizada-5-10 6.8.0-1018-gcp #20~22.04.1-Ubuntu SMP Thu Nov  7 18:30:15 UTC 2024 x86_64 x86_64 x86_64 GNU/Linux
    ```
    
- **Kafka Cluster**:
    - Local Kafka instance for Phase 1.
    - Remote Kafka cluster in later phases.
- **File Descriptors**:
    - Default (`ulimit -n 1024`) vs Optimized (`ulimit -n 65536`).

---

# Phase 1: Initial Testing

### **Testing Scenarios**
1. **Tokio (Hyper + Local Kafka)**:
    - Used Hyper (HTTP framework) with Tokio runtime.
    - Kafka integration using rdkafka.
2. **Tokio-Uring (TCP Stream + Local Kafka)**:
    - Used low-level TCP stream for handling HTTP-like requests.
    - Kafka integration using rdkafka BaseProducer.

---

# Phase 1: Initial Results

| Framework              | Setup                    | Throughput (req/sec) |
|-------------------------|--------------------------|-----------------------|
| **Tokio + Hyper**       | Local Kafka             | 132                  |
| **Tokio-Uring + TCP**   | Local Kafka             | 1500                 |

### **Observations**
- **Tokio-Uring** outperformed Hyper+Tokio by nearly **10x** in throughput.
- Hyper-based solution struggled to scale under load due to compatibility and resource bottlenecks.
- TCP Stream with Tokio-Uring showed high initial promise.

---

# Feedback After Phase 1

1. **HTTP Framework for Tokio-Uring**:
    - Writing a custom HTTP backend from scratch is not recommended.
    - Existing frameworks like Hyper cannot be used with Tokio-Uring (runtime compatibility issue).
      - Hyper within a tokio-uring runtime fails during runtime
      - Hyper with tokio and tokio-uring for kafka publish fails with cannot start runtime within runtime.
    - Could not find any HTTP webserver framework built on Tokio-Uring.
    - Most solutions using **Glommio** (another io_uring runtime) use a TCP connection stream.

2. **Explore rdkafka BaseProducer**:
    - BaseProducer is low-level and more performant.
    - Requires manual polling but enables higher throughput.

3. **Use Remote Kafka Cluster**:
    - Switch to a remote Kafka cluster to eliminate local resource contention.
    - Remote Kafka significantly improved performance.

4. **Even the Playing Field**:
    - For accurate comparison, switch both Tokio and Tokio-Uring implementations to use TCP streams instead of HTTP frameworks.

---

# Phase 2: Optimizations

1. **File Descriptor Limit**:
    - Increased `ulimit` from `1024` to `65536` to handle more open connections.

2. **Kafka Producer Configurations**:
    - Tuned producer configs for higher throughput:
        - Increased batch size.
        - Increased buffer memory.
        - Optimized `linger.ms` for batching.

3. **Remote Kafka Cluster**:
    - Used a dedicated remote Kafka cluster to reduce contention with local resources.

4. **Even Comparison**:
    - Both implementations used TCP streams instead of HTTP frameworks for fairness.

---

# Phase 2: Optimized Results

| Framework              | Setup          | Throughput (req/sec) |
|-------------------------|----------------|-----------------------|
| **Tokio**              | Remote Kafka   | 4459.9 – 4656.2      |
| **Tokio-Uring**        | Remote Kafka   | 3924.6 – 3939.5      |

---

# Observations: Tokio vs Tokio-Uring

### **Tokio**:
- Achieved higher throughput (**~4.5k req/sec**).
- Stable under sustained high load.
- Required increased file descriptor limits (`ulimit -n`).

### **Tokio-Uring**:
- Slightly lower throughput (**~3.9k req/sec**).
- Stalls under very high throughput:
    - Stops accepting new TCP connections.
    - No ephemeral ports assigned.
    - Requires a manual restart to unblock.

---

# Key Challenges: Tokio-Uring

1. **Application Stalling**:
    - At high throughput, Tokio-Uring stops accepting new connections.
    - Likely caused by `io_uring` backpressure under load.

2. **TCP Connection Limit**:
    - Stalling prevents ephemeral ports from being assigned.

3. **Lack of HTTP Framework**:
    - No existing HTTP frameworks available for Tokio-Uring.
    - Forced to rely on low-level TCP streams.

---

# Conclusions

### **Key Findings**:
1. **Tokio**:
    - Achieved higher throughput (~4.5k req/sec).
    - Stable and scales well under load.

2. **Tokio-Uring**:
    - Requires debugging for stalling and connection issues.

3. **Kafka Tuning**:
    - Switching to remote Kafka and tuning producer configs significantly improved performance.

---

# Next Steps

1. **Investigate Tokio-Uring Stalling**:
    - Debug `io_uring` backpressure and TCP connection limits.
    - Use profiling tools like `perf` or `tokio-console`.

2. **Explore Alternatives**:
    - Test other `io_uring` runtimes like **Glommio** for HTTP workloads.

---

