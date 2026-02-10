//! Cloud-init execution stages
//!
//! Stages run in order during boot:
//! 1. Local - before network (disk setup, growpart)
//! 2. Network - after network is up (metadata fetch, ssh keys)
//! 3. Config - configuration application (users, packages, files)
//! 4. Final - user scripts and final tasks

pub mod config;
pub mod final_stage;
pub mod local;
pub mod network;
