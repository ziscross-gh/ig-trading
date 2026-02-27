//! # IG Trading Engine
//!
//! A specialized high-frequency trading system for IG Markets, featuring
//! multi-strategy ensemble voting, adaptive risk management, and real-time dashboard IPC.

/// IG API client and types
pub mod api;
/// Data storage and history management
pub mod data;
/// Core engine loop and state management
pub mod engine;
/// Technical indicators and signal generation
pub mod indicators;
/// Inter-process communication (HTTP, WebSocket, Events)
pub mod ipc;
/// Strategy optimization and performance learning
pub mod learning;
/// Notification services (Telegram, etc.)
pub mod notifications;
/// Hard-gated risk management and position sizing
pub mod risk;
/// Trading strategies and ensemble logic
pub mod strategy;
