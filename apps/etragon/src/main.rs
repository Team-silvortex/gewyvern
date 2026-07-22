use etragon::{
    LearningBackend, LearningBackendConfig, NativeLearningBackend, NativeLearningConfig,
    PythonWorkerConfig, default_python_worker_script, spawn_learning_backend,
    with_learning_backend, with_python_worker,
};
use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod cli_commands;
mod cli_memory;
mod cli_options;
mod daemon_auth;
mod daemon_capabilities;
mod daemon_memory_routes;
mod daemon_request;
mod daemon_routes;
mod daemon_server;
mod daemon_state;
mod daemon_views;
mod federation;
mod json_support;
mod learning_hints;
mod learning_output;
mod memory_transfer;
mod recommendation;

use cli_commands::*;
use cli_memory::*;
use cli_options::*;
use daemon_auth::*;
use daemon_capabilities::*;
use daemon_memory_routes::*;
use daemon_request::*;
use daemon_routes::*;
use daemon_server::*;
use daemon_state::*;
use daemon_views::*;
use federation::*;
use json_support::*;
use learning_hints::*;
use learning_output::*;
use memory_transfer::*;
use recommendation::*;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match run_cli(&args) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", output);
            }
        }
        Err(message) => {
            eprintln!("{}", message);
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests;
