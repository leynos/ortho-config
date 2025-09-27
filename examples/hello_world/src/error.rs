//! Error types for the `hello_world` example.
//!
//! This module centralises configuration and validation failures so `main`
//! can report user-friendly errors without exposing internal details.
//!
//! `HelloWorldError` wraps the derive layer errors alongside local validation
//! issues so the binary renders concise, actionable diagnostics.
use std::sync::Arc;
use thiserror::Error;

/// Errors raised by the hello world example.
#[derive(Debug, Error)]
pub enum HelloWorldError {
    /// Wraps configuration parsing failures from `ortho_config`.
    #[error("failed to load configuration: {0}")]
    Configuration(#[from] Arc<ortho_config::OrthoError>),
    /// Bubbles up validation issues detected before executing the command.
    #[error(transparent)]
    Validation(#[from] ValidationError),
}

/// Validation issues detected while resolving global options.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    /// No greeting words were provided.
    #[error("at least one salutation must be provided")]
    MissingSalutation,
    /// A provided salutation collapsed to nothing after trimming.
    #[error("salutations must contain visible characters (index {0})")]
    BlankSalutation(usize),
    /// Mutually exclusive delivery modes were enabled simultaneously.
    #[error("cannot combine --is-excited with --is-quiet")]
    ConflictingDeliveryModes,
    /// Greeting punctuation collapsed to nothing after trimming.
    #[error("greeting punctuation must contain visible characters")]
    BlankPunctuation,
    /// Greeting preamble collapsed to nothing after trimming.
    #[error("preambles must contain visible characters when supplied")]
    BlankPreamble,
    /// Farewell phrase collapsed to nothing after trimming.
    #[error("farewell messages must contain visible characters")]
    BlankFarewell,
    /// Reminder durations must be positive.
    #[error("reminder minutes must be greater than zero")]
    ReminderOutOfRange,
    /// Gift descriptions collapsed to nothing after trimming.
    #[error("gift descriptions must contain visible characters")]
    BlankGift,
}
