mod bindings;
mod callback;
mod client;
mod log;
mod logging;
mod peer;
mod peer_manager;
mod websocket;

use futures::channel::mpsc;
use gloo_net::websocket::Message;
use std::{cell::RefCell, rc::Rc};

pub(crate) type WsSenderState = Rc<RefCell<Option<mpsc::Sender<Message>>>>;

pub use client::*;
