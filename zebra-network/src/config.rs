use std::{
    collections::HashSet,
    net::{SocketAddr, ToSocketAddrs},
    string::String,
    time::Duration,
};

use zebra_chain::parameters::Network;

/// Configuration for networking code.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// The address on which this node should listen for connections.
    pub listen_addr: SocketAddr,

    /// The network to connect to.
    pub network: Network,

    /// A list of initial peers for the peerset when operating on
    /// mainnet.
    pub initial_mainnet_peers: HashSet<String>,

    /// A list of initial peers for the peerset when operating on
    /// testnet.
    pub initial_testnet_peers: HashSet<String>,

    /// The initial target size for the peer set.
    ///
    /// If you have a slow network connection, and Zebra is having trouble
    /// syncing, try reducing the peer set size. You can also reduce the peer
    /// set size to reduce Zebra's bandwidth usage.
    pub peerset_initial_target_size: usize,

    /// How frequently we attempt to connect to a new peer.
    pub new_peer_interval: Duration,
}

impl Config {
    fn parse_peers<S: ToSocketAddrs>(peers: HashSet<S>) -> HashSet<SocketAddr> {
        peers
            .iter()
            .flat_map(|s| s.to_socket_addrs())
            .flatten()
            .collect()
    }

    /// Get the initial seed peers based on the configured network.
    pub fn initial_peers(&self) -> HashSet<SocketAddr> {
        match self.network {
            Network::Mainnet => Config::parse_peers(self.initial_mainnet_peers.clone()),
            Network::Testnet => Config::parse_peers(self.initial_testnet_peers.clone()),
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        let mainnet_peers = [
            "dnsseed.z.cash:8233",
            "dnsseed.str4d.xyz:8233",
            "mainnet.seeder.zfnd.org:8233",
            "mainnet.is.yolo.money:8233",
        ]
        .iter()
        .map(|&s| String::from(s))
        .collect();

        let testnet_peers = [
            "dnsseed.testnet.z.cash:18233",
            "testnet.seeder.zfnd.org:18233",
            "testnet.is.yolo.money:18233",
        ]
        .iter()
        .map(|&s| String::from(s))
        .collect();

        Config {
            listen_addr: "0.0.0.0:8233"
                .parse()
                .expect("Hardcoded address should be parseable"),
            network: Network::Mainnet,
            initial_mainnet_peers: mainnet_peers,
            initial_testnet_peers: testnet_peers,
            new_peer_interval: Duration::from_secs(60),

            // The default peerset target size should be large enough to ensure
            // nodes have a reliable set of peers. But it should also be limited
            // to a reasonable size, to avoid queueing too many in-flight block
            // downloads. A large queue of in-flight block downloads can choke a
            // constrained local network connection.
            //
            // We assume that Zebra nodes have at least 10 Mbps bandwidth.
            // Therefore, a maximum-sized block can take up to 2 seconds to
            // download. So a full default peer set adds up to 100 seconds worth
            // of blocks to the queue.
            //
            // But the peer set for slow nodes is typically much smaller, due to
            // the handshake RTT timeout.
            peerset_initial_target_size: 50,
        }
    }
}
