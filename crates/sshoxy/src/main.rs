use std::{io::ErrorKind, sync::Arc};

use clap::Parser;
use russh::{
    ChannelId, Preferred,
    server::{self, Server as _},
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();
    let host_key =
        match russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519) {
            Ok(key) => key,
            Err(e) => {
                log::error!("Failed to generate host key: {e}");
                std::process::exit(1);
            }
        };
    let config = russh::server::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs(args.timeout)),
        auth_rejection_time: std::time::Duration::from_secs(3),
        auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
        keys: vec![host_key],
        preferred: Preferred {
            ..Preferred::default()
        },
        ..Default::default()
    };
    let config = Arc::new(config);
    let mut sh = Server {
        welcome_message: args.welcome_message,
        command: args.command,
    };

    let socket = match TcpListener::bind((args.address.clone(), args.port)).await {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            log::error!("Port {} already in use", args.port);
            std::process::exit(1);
        }
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            log::error!("Permission denied to bind to port {}", args.port);
            std::process::exit(1);
        }
        Err(e) => {
            log::error!(
                "Failed to bind to host: {} port :{}, reason: {}",
                args.address,
                args.port,
                e
            );
            std::process::exit(1);
        }
    };
    let server = sh.run_on_socket(config, &socket);

    if let Err(e) = server.await {
        log::error!("SSH server terminated with error: {e}");
        std::process::exit(1);
    }
}

/// An ssh server that runs a specified program for each user that logs in
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The port to listen on
    #[clap(short, long, required = true, default_value = "2222")]
    port: u16,

    /// The welcome message to display to users when they log in
    #[clap(short, long)]
    welcome_message: Option<String>,

    /// The program to run for each user that logs in
    #[clap(short, long, required = true)]
    command: String,

    /// ssh timeout in seconds
    #[clap(short, long, default_value = "3600")]
    timeout: u64,

    /// the host name to bind to, default is 0.0.0.0
    #[clap(short, long, default_value = "0.0.0.0")]
    address: String,
}

#[derive(Clone)]
#[allow(dead_code)]
struct Server {
    welcome_message: Option<String>,
    command: String,
}

impl server::Handler for Server {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        log::debug!("Auth publickey for user: {}, key: {:?}", user, public_key);
        Ok(server::Auth::Accept)
    }

    async fn auth_succeeded(&mut self, _session: &mut server::Session) -> Result<(), Self::Error> {
        log::debug!("Authentication succeeded!");
        Ok(())
    }

    async fn authentication_banner(&mut self) -> Result<Option<String>, Self::Error> {
        Ok(self.welcome_message.clone())
    }

    async fn channel_open_session(
        &mut self,
        _channel: russh::Channel<server::Msg>,
        _reply: server::ChannelOpenHandle,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if data == [3] {
            return Err(russh::Error::Disconnect);
        }
        Ok(())
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        log::info!("TCP/IP forward request for {}:{}", address, port);
        Ok(true)
    }
}

impl server::Server for Server {
    type Handler = Self;

    fn new_client(&mut self, peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
        if let Some(addr) = peer_addr {
            log::debug!("New client connected from:{}", addr.ip());
        }
        self.clone()
    }

    fn handle_session_error(&mut self, error: <Self::Handler as server::Handler>::Error) {
        log::error!("Session error: {:?}", error);
    }
}
