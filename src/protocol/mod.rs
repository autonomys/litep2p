// Copyright 2023 litep2p developers
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! Protocol-related defines.

use crate::{
    error::Error,
    peer_id::PeerId,
    substream::Substream,
    transport::{Connection, TransportEvent},
    types::protocol::ProtocolName as NewProtocolName,
};

use futures::Stream;
use tokio::sync::mpsc::{Receiver, Sender};

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

pub mod libp2p;
pub mod notification;
pub mod notification_new;
pub mod request_response;
pub mod request_response_new;

/// Commands sent by different protocols to `Litep2p`.
#[derive(Debug)]
pub enum TransportCommand {
    /// Open substream to remote peer.
    OpenSubstream {
        /// Protocol.
        protocol: String,

        /// Remote peer ID.
        peer: PeerId,
    },
}

#[derive(Debug, Clone)]
pub enum ProtocolName {
    /// Static protocol name.
    Static(&'static str),
}

impl Display for ProtocolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl From<&'static str> for ProtocolName {
    fn from(value: &'static str) -> Self {
        ProtocolName::Static(value)
    }
}

/// Libp2p protocol configuration.
#[derive(Debug)]
pub struct Libp2pProtocol {
    /// Protocol name.
    name: ProtocolName,
}

impl Libp2pProtocol {
    /// Create new [`Libp2pProtocol`].
    pub fn new(name: ProtocolName) -> Self {
        Self { name }
    }

    /// Get the name of the protocol.
    pub fn name(&self) -> &ProtocolName {
        &self.name
    }

    /// Get the name as `String`.
    pub fn to_string(&self) -> String {
        println!("convert {} to string", self.name);
        self.name.to_string()
    }
}

/// Notification protocol configuration.
#[derive(Debug)]
pub struct NotificationProtocol {
    /// Protocol name.
    name: ProtocolName,
}

impl NotificationProtocol {
    /// Create new [`NotificationProtocol`].
    pub fn new(name: ProtocolName) -> Self {
        Self { name }
    }

    /// Get the name of the protocol.
    pub fn name(&self) -> &ProtocolName {
        &self.name
    }

    /// Get the name as `String`.
    pub fn to_string(&self) -> String {
        self.name.to_string()
    }
}

/// Events received from connections that relevant to the execution of a user protocol.
pub enum ExecutionEvent<S: Substream> {
    /// Connection established to remote peer.
    ConnectionEstablished {
        /// Peer ID.
        peer: PeerId,
    },

    /// Connection closed to remote peer.
    ConnectionClosed {
        /// Peer ID.
        peer: PeerId,
    },

    /// Substream opened to remote peer.
    SubstreamOpened {
        /// Peer ID.
        peer: PeerId,

        /// Opened substream.
        substream: S,
    },

    /// Failed to open substream.
    SubstreamOpenFailure {
        /// Peer ID.
        peer: PeerId,

        /// Error that occurred.
        error: Error,
    },
}

#[async_trait::async_trait]
pub trait SubstreamService {
    /// Open substream.
    async fn open_subtream(&mut self, peer: PeerId) -> crate::Result<()>;

    /// Poll next event from the execution context.
    async fn next_event<S: Substream>(&mut self) -> Option<ExecutionEvent<S>>;
}

pub trait Codec {}
pub type EventStream = ();

/// TODO: documentation
pub trait ProtocolBuilder {
    type Protocol: Protocol;

    /// Get protocol name.
    fn protocol_name(&self) -> NewProtocolName;

    /// Build `Protocol`.
    fn build(self, sender: Sender<()>) -> Self::Protocol;
}

#[async_trait::async_trait]
pub trait Protocol {
    type Event: Debug;

    /// Start the protocol runner.
    async fn run(self);
}

/// Events emitted by a connection to protocols.
pub enum ConnectionEvent {
    /// Substream opened for `peer`.
    SubstreamOpened {
        /// Peer ID.
        peer: PeerId,

        /// Substream.
        substream: Box<dyn Substream>,
    },

    /// Failed to open substream.
    SubstreamOpenFailure {
        /// Peer Id.
        peer: PeerId,

        /// Error.
        error: Error,
    },
}

/// Supported protocol information.
///
/// Each connection gets a copy of [`ProtocolInfo`] which allows it to interact
/// directly with installed protocols.
pub struct ProtocolInfo {
    protocols: HashMap<NewProtocolName, Sender<ConnectionEvent>>,
    rx: Receiver<NewProtocolName>,
}

impl ProtocolInfo {
    /// Create new [`ProtocolInfo`].
    pub fn new(
        protocols: HashMap<NewProtocolName, Sender<ConnectionEvent>>,
        rx: Receiver<NewProtocolName>,
    ) -> Self {
        Self { protocols, rx }
    }

    /// Report to `protocol` that substream was opened for `peer`.
    pub async fn report_substream_open(
        &mut self,
        protocol: NewProtocolName,
        peer: PeerId,
        substream: Box<dyn Substream>,
    ) -> crate::Result<()> {
        match self.protocols.get_mut(&protocol) {
            Some(sender) => sender
                .send(ConnectionEvent::SubstreamOpened { peer, substream })
                .await
                .map_err(From::from),
            None => Err(Error::ProtocolNotSupported(protocol.to_string())),
        }
    }

    /// Report to `protocol` that connection failed to open substream for `peer`.
    pub async fn report_substream_open_failure(
        &mut self,
        protocol: NewProtocolName,
        peer: PeerId,
        error: Error,
    ) -> crate::Result<()> {
        match self.protocols.get_mut(&protocol) {
            Some(sender) => sender
                .send(ConnectionEvent::SubstreamOpenFailure { peer, error })
                .await
                .map_err(From::from),
            None => Err(Error::ProtocolNotSupported(protocol.to_string())),
        }
    }

    /// Poll next substream open query from one of the installed protocols.
    pub async fn poll_next(&mut self) -> Option<NewProtocolName> {
        self.rx.recv().await
    }
}
