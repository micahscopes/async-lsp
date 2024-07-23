//! This module provides the `CanHandle` trait, which allows services to indicate
//! their ability to handle specific message types. This functionality is particularly
//! useful for creating layers that compose multiple Services/LspServices and need
//! to reason about where to route messages.
//!
//! The module includes an automatic implementation of `CanHandle` for `BoxLspService`,
//! allowing boxed services to seamlessly integrate with this routing mechanism.

/// Indicates whether a service can handle a specific message type.
///
/// This trait allows services to communicate their ability to process
/// particular methods or message types.
pub trait CanHandle<Message> {
    /// Returns `true` if the service can handle the given message.
    fn can_handle(&self, e: &Message) -> bool;
}
