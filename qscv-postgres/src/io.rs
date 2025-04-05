use std::io;

use crate::{
    message::{frontend::Startup, BackendProtocol, FrontendProtocol},
    Result,
};

/// A buffered stream which can send and receive postgres message
pub trait PostgresIo {
    /// send message to the backend
    ///
    /// this does not actually write to the underlying io,
    /// instead implementor should buffer it
    ///
    /// use [`flush`][`PostgresIo::flush`] to actually send the message
    fn send<F: FrontendProtocol>(&mut self, message: F);

    /// send [`Startup`] message to the backend
    ///
    /// For historical reasons, the very first message sent by the client (the startup message)
    /// has no initial message-type byte.
    ///
    /// Thus, [`Startup`] does not implement [`FrontendProtocol`]
    fn send_startup(&mut self, startup: Startup);

    /// actually write buffered messages to underlying io
    fn flush(&mut self) -> impl Future<Output = io::Result<()>>;

    /// receive a backend message
    fn recv<B: BackendProtocol>(&mut self) -> impl Future<Output = Result<B>>;
}

