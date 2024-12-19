mod main_hyper_iouring_remote_kafka_future_producer;
mod main_hyper_local_kafka_base_producer;
mod main_hyper_local_kafka_future_producer;
mod main_hyper_remote_kafka_base_producer;
mod main_hyper_tokio_remote_kafka_future_producer;
mod main_tcp_local_kafka_base_producer;
mod main_tcp_local_kafka_future_producer;
mod main_tcp_remote_kafka_base_producer;
mod main_tcp_remote_kafka_future_producer;
mod main_tcp_tokio_remote_kafka_base_producer;

fn main() {
    // main_tcp_remote_kafka_base_producer::main().unwrap();
    // main_tcp_tokio_remote_kafka_base_producer::main().unwrap();
    main_tcp_remote_kafka_future_producer::main().unwrap()
}
