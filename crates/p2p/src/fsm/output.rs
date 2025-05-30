//! Protocol output capabilities.
//!
//! See [`Outbox`] type.
//!
//! Each sub-protocol, eg. the "ping" or "handshake" protocols are given a copy of this outbox
//! with specific capabilities, eg. peer disconnection, message sending etc. to
//! communicate with the network.
use bitcoin::p2p::Address;
use bitcoin::p2p::message_network::VersionMessage;
use lrc20_types::messages::p2p::{NetworkMessage, RawNetworkMessage};
use lrc20_types::network::Network;
use prometheus::{IntCounterVec, register_int_counter_vec};
use std::sync::{Arc, Mutex};
use std::{collections::VecDeque, net};
use tracing::{debug, trace};

use crate::{
    fsm::event::Event,
    fsm::handler::{DisconnectReason, PeerId},
    net::LocalDuration,
};

lazy_static::lazy_static! {
    static ref GOSSIP_MESSAGE_SENT_TOTAL: IntCounterVec = register_int_counter_vec!(
        "lrc20_gossip_message_sent_total",
        "Total number of sent gossip messages",
        &["message"]
    ).unwrap();
}

/// Output of a state transition of the `Protocol` state machine.
pub type Io = crate::net::Io<RawNetworkMessage, Event, DisconnectReason>;

impl From<Event> for Io {
    fn from(event: Event) -> Self {
        Io::Event(event)
    }
}

/// Ability to connect to peers.
pub trait Connect {
    /// Connect to peer.
    fn connect(&self, addr: net::SocketAddr, timeout: LocalDuration);
}

/// Ability to disconnect from peers.
pub trait Disconnect {
    /// Disconnect from peer.
    fn disconnect(&self, addr: net::SocketAddr, reason: DisconnectReason);
}

/// The ability to set a timer.
pub trait SetTimer {
    /// Ask to be woken up in a predefined amount of time.
    fn set_timer(&self, duration: LocalDuration) -> &Self;
}

/// Bitcoin wire protocol.
pub trait Wire<E> {
    /// Emit an event.
    fn event(&self, event: E);

    // Handshake messages //////////////////////////////////////////////////////

    /// Send a `version` message.
    fn version(&mut self, addr: PeerId, msg: VersionMessage);

    /// Send a `verack` message.
    fn verack(&mut self, addr: PeerId) -> &mut Self;

    /// Send a BIP-339 `wtxidrelay` message.
    fn wtxid_relay(&mut self, addr: PeerId) -> &mut Self;

    /// Send a BIP-339 `ltxidrelay` message.
    fn ltxid_relay(&mut self, addr: PeerId) -> &mut Self;

    /// Send a BIP-339 `ltxidack` message.
    fn ltxidack(&mut self, addr: PeerId) -> &mut Self;

    // Ping/pong ///////////////////////////////////////////////////////////////

    /// Send a `ping` message.
    fn ping(&mut self, addr: net::SocketAddr, nonce: u64) -> &Self;

    /// Send a `pong` message.
    fn pong(&mut self, addr: net::SocketAddr, nonce: u64) -> &Self;

    // Addresses //////////////////////////////////////////////////////////////

    /// Send a `getaddr` message.
    fn get_addr(&mut self, addr: PeerId);

    /// Send an `addr` message.
    fn addr(&mut self, addr: PeerId, addrs: Vec<(u32, Address)>);
}

/// Holds protocol outputs and pending I/O.
#[derive(Debug, Clone)]
pub struct Outbox {
    /// Bitcoin network.
    network: Network,
    /// Output queue.
    pub outbound: Arc<Mutex<VecDeque<Io>>>,
}

impl Default for Outbox {
    fn default() -> Self {
        Self::new(Network::Bitcoin)
    }
}

impl Iterator for Outbox {
    type Item = Io;

    /// Get the next item in the outbound queue.
    fn next(&mut self) -> Option<Io> {
        self.outbound.lock().unwrap().pop_front()
    }
}

impl Outbox {
    /// Create a new channel.
    pub fn new(network: Network) -> Self {
        Self {
            network,
            outbound: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Push an output to the channel.
    pub fn push(&self, output: Io) {
        self.outbound.lock().unwrap().push_back(output);
    }

    /// Push a message to the channel.
    pub fn message(&mut self, addr: PeerId, payload: NetworkMessage) -> &Self {
        GOSSIP_MESSAGE_SENT_TOTAL
            .with_label_values(&[payload.command()])
            .inc();

        if let NetworkMessage::Inv { .. } = payload {
            trace!(target: "p2p", "Sending {:?} to {}", payload, addr);
        } else {
            debug!(target: "p2p", "Sending {:?} to {}", payload, addr);
        }

        self.push(Io::Write(
            addr,
            RawNetworkMessage {
                magic: self.network.magic(),
                payload,
            },
        ));

        self
    }

    /// Push an event to the channel.
    pub fn event(&self, event: Event) {
        self.push(Io::Event(event));
    }
}

impl Disconnect for Outbox {
    fn disconnect(&self, addr: net::SocketAddr, reason: DisconnectReason) {
        debug!(target: "p2p", "Disconnecting from {}: {}", addr, reason);

        self.push(Io::Disconnect(addr, reason));
    }
}

impl SetTimer for Outbox {
    fn set_timer(&self, duration: LocalDuration) -> &Self {
        self.push(Io::SetTimer(duration));
        self
    }
}

impl Connect for Outbox {
    fn connect(&self, addr: net::SocketAddr, timeout: LocalDuration) {
        self.push(Io::Connect(addr));
        self.push(Io::SetTimer(timeout));
    }
}

impl<E: Into<Event> + std::fmt::Display> Wire<E> for Outbox {
    fn event(&self, event: E) {
        debug!(target: "p2p", "{}", &event);
        self.event(event.into());
    }

    fn version(&mut self, addr: PeerId, msg: VersionMessage) {
        self.message(addr, NetworkMessage::Version(msg));
    }

    fn verack(&mut self, addr: PeerId) -> &mut Self {
        self.message(addr, NetworkMessage::Verack);
        self
    }

    fn wtxid_relay(&mut self, addr: PeerId) -> &mut Self {
        self.message(addr, NetworkMessage::WtxidRelay);
        self
    }

    fn ltxid_relay(&mut self, addr: PeerId) -> &mut Self {
        self.message(addr, NetworkMessage::LtxidRelay);
        self
    }

    fn ltxidack(&mut self, addr: PeerId) -> &mut Self {
        self.message(addr, NetworkMessage::Ltxidack);
        self
    }

    fn ping(&mut self, addr: net::SocketAddr, nonce: u64) -> &Self {
        self.message(addr, NetworkMessage::Ping(nonce));
        self
    }

    fn pong(&mut self, addr: net::SocketAddr, nonce: u64) -> &Self {
        self.message(addr, NetworkMessage::Pong(nonce));
        self
    }

    fn get_addr(&mut self, addr: PeerId) {
        self.message(addr, NetworkMessage::GetAddr);
    }

    fn addr(&mut self, addr: PeerId, addrs: Vec<(u32, Address)>) {
        self.message(addr, NetworkMessage::Addr(addrs));
    }
}
