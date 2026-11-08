//! Craftski: ranks WoW Forever recipes and vendor flips by auction-house profit.
// The crate is a library only so the desktop binary and the web entry share it, not a public API.
#![allow(clippy::len_without_is_empty, clippy::new_without_default)]

pub mod blp;
pub mod breakdown;
pub mod cache;
pub mod charts;
pub mod copper;
pub mod flip_rows;
pub mod flips;
pub mod foreverdb;
pub mod icons;
pub mod items;
pub mod market;
pub mod menu;
pub mod merchants;
pub mod overview;
pub mod platform;
pub mod prices;
pub mod profit;
pub mod public_data;
pub mod recipes;
pub mod rows;
pub mod settings;
pub mod sheets;
pub mod skin;
pub mod theme;
pub mod tooltip;
pub mod vendor;
pub mod view;
pub mod wow;
#[cfg(target_arch = "wasm32")]
mod web;
