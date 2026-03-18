// Copyright 2026 Carlos Escobar-Valbuena
// SPDX-License-Identifier: Apache-2.0

//! Arcan runtime adapter for Symphony.
//!
//! Replaces CLI subprocess spawning with Arcan HTTP session API.
//! When `runtime.kind: arcan` is configured in WORKFLOW.md,
//! Symphony dispatches work through the Arcan daemon instead of
//! spawning local subprocesses.

pub mod client;
pub mod event_mapper;
pub mod runner;
