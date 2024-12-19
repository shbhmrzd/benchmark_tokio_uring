use std::env;

/// This module contains logic to prepare the test environment
/// with necessary external dependencies, e.g., Kafka.
use testcontainers::clients::Cli;
use testcontainers::core::ExecCommand;
use testcontainers::core::WaitFor;
use testcontainers::images::generic::GenericImage;
use testcontainers::Container;

// These are container internal ports
static ZK_PORT: u16 = 2181;
static KAFKA_BROKER_PORT: u16 = 9092;
static KAFKA_LISTENER_PORT: u16 = 9094;

/// Start Kafka and Zookeeper containers and returns the bootstrap servers.
pub fn set_up_kafka(
    docker: &Cli,
) -> (
    String,
    Option<testcontainers::Container<'_, GenericImage>>,
    Option<testcontainers::Container<'_, GenericImage>>,
) {
    match env::var("CIRCLECI") {
        // Kafka is already running in CircleCI
        Ok(_) => (format!("127.0.0.1:{}", KAFKA_BROKER_PORT), None, None),
        Err(_) => {
            // Start Zookeeper container
            let zk_container = docker.run(zk_image(ZK_PORT));

            // Start Kafka container
            let kafka_container = docker.run(kafka_image(
                KAFKA_LISTENER_PORT,
                KAFKA_BROKER_PORT,
                format!("{}:{}", zk_container.get_bridge_ip_address(), ZK_PORT).as_str(),
            ));

            // Get external Kafka listener port
            let external_kafka_listener_port =
                kafka_container.get_host_port_ipv4(KAFKA_LISTENER_PORT);

            // Make Kafka accessible outside the container
            alter_advertised_listener_port(
                external_kafka_listener_port,
                KAFKA_BROKER_PORT,
                &kafka_container,
            );

            // Return bootstrap servers
            let bootstrap_servers = format!("127.0.0.1:{}", external_kafka_listener_port);
            (bootstrap_servers, Some(kafka_container), Some(zk_container))
        }
    }
}

pub fn zk_image(port: u16) -> GenericImage {
    GenericImage::new("confluentinc/cp-zookeeper", "latest")
        .with_env_var("ZOOKEEPER_CLIENT_PORT", port.to_string())
}

pub fn kafka_image(
    kafka_listener_port: u16,
    kafka_broker_port: u16,
    zk_connect_string: &str,
) -> GenericImage {
    GenericImage::new("wurstmeister/kafka", "latest")
        .with_wait_for(WaitFor::message_on_stdout(
            "started (kafka.server.KafkaServer)",
        ))
        // Kafka broker ID
        .with_env_var("KAFKA_BROKER_ID", "1")
        // Kafka listeners
        .with_env_var(
            "KAFKA_LISTENERS",
            format!(
                "PLAINTEXT://0.0.0.0:{},BROKER://0.0.0.0:{}",
                kafka_listener_port, kafka_broker_port
            ),
        )
        // Advertised listeners
        .with_env_var(
            "KAFKA_ADVERTISED_LISTENERS",
            format!(
                "PLAINTEXT://127.0.0.1:{},BROKER://127.0.0.1:{}",
                kafka_listener_port, kafka_broker_port
            ),
        )
        // Listener security protocol map
        .with_env_var(
            "KAFKA_LISTENER_SECURITY_PROTOCOL_MAP",
            "PLAINTEXT:PLAINTEXT,BROKER:PLAINTEXT",
        )
        // Inter-broker listener name
        .with_env_var("KAFKA_INTER_BROKER_LISTENER_NAME", "BROKER")
        // Zookeeper connection
        .with_env_var("KAFKA_ZOOKEEPER_CONNECT", zk_connect_string)
        // Topic replication factor
        .with_env_var("KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR", "1")
        // Default number of partitions for topics
        .with_env_var("KAFKA_NUM_PARTITIONS", "4") // Increase default partitions to 4
        // Increase memory allocation to 2GB
        .with_env_var("KAFKA_HEAP_OPTS", "-Xmx3G -Xms3G")
        // Expose the Kafka listener port
        .with_exposed_port(kafka_listener_port)
}

/// Until Kafka container starts, we do not know the port mappings.
/// We actually do not want to know either, because a static mapping
/// could potentially cause port conflicts (the testcontainers lib
/// does not support it on purpose).
///
/// This function alters the advertised listener port for external
/// connection from internal port to external port.
pub fn alter_advertised_listener_port(
    new_kafka_listener_port: u16,
    kafka_broker_port: u16,
    kafka_container: &Container<'_, GenericImage>,
) {
    let alter_advertised_listener_port = format!(
        "kafka-configs.sh --alter --bootstrap-server 0.0.0.0:{} --entity-type brokers --entity-name 1 \
        --add-config advertised.listeners=[PLAINTEXT://127.0.0.1:{},BROKER://127.0.0.1:{}]",
        kafka_broker_port,
        new_kafka_listener_port,
        kafka_broker_port
    );

    kafka_container.exec(ExecCommand {
        cmd: alter_advertised_listener_port,
        ready_conditions: vec![WaitFor::message_on_stdout("Updated broker 1 at path")],
    });
}
