use std::{io::ErrorKind, sync::Arc};

use clap::Parser;
use russh::{
    ChannelId, MethodKind, MethodSet, Preferred,
    server::{self, Server as _},
};
use tokio::net::TcpListener;

// sshoxy arcade
// - snake
// - alien destroyer
// - ball hit wall and remove
// - tetris
// - battleship

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let host_key = load_or_generate_host_key(args.host_key.as_deref())?;
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
        no_auth: args.no_auth,
        state: SessionState {
            user: String::new(),
            public_key: None,
        },
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
    Ok(())
}

fn load_or_generate_host_key(
    explicit_path: Option<&str>,
) -> Result<russh::keys::PrivateKey, Box<dyn std::error::Error>> {
    if let Some(path) = explicit_path {
        return russh::keys::load_secret_key(path, None).map_err(|e| {
            log::error!("Failed to load host key from {path}: {e}");
            e.into()
        });
    }

    let project_dirs =
        directories::ProjectDirs::from("com", "sshoxy", "sshoxy").ok_or_else(|| {
            log::error!("Failed to resolve project data directory");
            std::io::Error::other("Failed to get project directory")
        })?;
    let data_dir = project_dirs.data_dir();
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join("host_key.pem");

    if path.exists() {
        return russh::keys::load_secret_key(&path, None).map_err(|e| {
            log::error!(
                "Failed to load cached host key from {}: {e}",
                path.display()
            );
            e.into()
        });
    }

    log::debug!("Generating new Ed25519 host key at {}", path.display());
    let key = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)?;
    key.write_openssh_file(&path, russh::keys::ssh_key::LineEnding::LF)?;
    Ok(key)
}

/// An ssh server that runs a specified program for each user that logs in
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The port to listen on
    #[clap(short, long, required = true, default_value = "2222")]
    port: u16,

    /// host key file to use for the server, if not specified a random key will be generated
    #[clap(long)]
    host_key: Option<String>,

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

    /// no authentication, allow any user to log in without a public key
    #[clap(long)]
    no_auth: bool,
}

#[derive(Clone)]
struct SessionState {
    pub user: String,
    pub public_key: Option<russh::keys::ssh_key::PublicKey>,
}

#[derive(Clone)]
struct Server {
    pub welcome_message: Option<String>,
    pub command: String,
    pub state: SessionState,
    pub no_auth: bool,
}

impl Server {
    pub fn set_user(&mut self, user: &str) {
        let new_state = SessionState {
            user: String::from(user),
            public_key: self.state.public_key.clone(),
        };
        self.state = new_state;
    }

    pub fn set_publickey(&mut self, public_key: russh::keys::ssh_key::PublicKey) {
        let new_state = SessionState {
            user: self.state.user.clone(),
            public_key: Some(public_key),
        };
        self.state = new_state;
    }
}

impl server::Handler for Server {
    type Error = russh::Error;

    async fn auth_none(&mut self, user: &str) -> Result<server::Auth, Self::Error> {
        log::debug!("Auth none for user: {}", user);
        if self.no_auth {
            self.set_user(user);
            return Ok(server::Auth::Accept);
        }
        let mut method_set = MethodSet::empty();
        method_set.push(MethodKind::PublicKey);
        Ok(server::Auth::Reject {
            proceed_with_methods: Some(method_set),
            partial_success: false,
        })
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        log::debug!("Auth publickey for user: {}, key: {:?}", user, public_key);
        self.set_user(user);
        self.set_publickey(public_key.clone());
        Ok(server::Auth::Accept)
    }

    async fn auth_succeeded(&mut self, _session: &mut server::Session) -> Result<(), Self::Error> {
        log::debug!("Auth succeeded");
        Ok(())
    }

    async fn authentication_banner(&mut self) -> Result<Option<String>, Self::Error> {
        Ok(self.welcome_message.clone())
    }

    async fn channel_open_session(
        &mut self,
        _channel: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
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

    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        _channel: ChannelId,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn pty_request(
        &mut self,
        _channel: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn shell_request(
        &mut self,
        _channel: ChannelId,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn exec_request(
        &mut self,
        _channel: ChannelId,
        _data: &[u8],
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn signal(
        &mut self,
        _channel: ChannelId,
        _signal: russh::Sig,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
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
