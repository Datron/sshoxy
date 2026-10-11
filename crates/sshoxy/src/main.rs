use std::{collections::HashMap, io::ErrorKind, sync::Arc};

use clap::Parser;
use portable_pty::PtySize;
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
    let command = shell_words::split(&args.command).map_err(|e| {
        log::error!("Failed to parse command: {e}");
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid command")
    })?;
    let mut sh = Server::new(args.welcome_message.clone(), command.clone(), args.no_auth);

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

struct Session {
    pub user: String,
    pub public_key: Option<russh::keys::ssh_key::PublicKey>,
    pub term: String,
    pub size: PtySize,
    pub writers: HashMap<ChannelId, Box<dyn std::io::Write + Send>>,
    pub killers: HashMap<ChannelId, Box<dyn portable_pty::ChildKiller + Send + Sync>>,
    pub masters: HashMap<ChannelId, Box<dyn portable_pty::MasterPty + Send>>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            user: String::new(),
            public_key: None,
            term: String::new(),
            size: PtySize::default(),
            writers: HashMap::new(),
            killers: HashMap::new(),
            masters: HashMap::new(),
        }
    }
}

struct Server {
    pub welcome_message: Option<String>,
    pub command: Vec<String>,
    pub session: Session,
    pub no_auth: bool,
}

impl Server {
    pub fn new(welcome_message: Option<String>, command: Vec<String>, no_auth: bool) -> Self {
        Self {
            welcome_message,
            command,
            session: Session::default(),
            no_auth,
        }
    }

    async fn spawn_shell(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), russh::Error> {
        let mut parts = self.command.iter();
        let prog = parts.next().ok_or_else(|| {
            log::error!(
                "No command specified to run for user: {}",
                self.session.user
            );
            russh::Error::ChannelOpenFailure(russh::ChannelOpenFailure::Other {
                code: 1,
                reason: "No command specified to run for this user".to_string(),
            })
        })?;
        let pair = portable_pty::native_pty_system()
            .openpty(self.session.size.clone())
            .map_err(|e| {
                log::error!("Could not setup PTY due to: {}", e);
                russh::Error::HUP
            })?;
        let mut cmd = portable_pty::CommandBuilder::new(prog);
        for a in parts {
            cmd.arg(a.as_str());
        }
        cmd.env("SSHOXY_USER", self.session.user.clone());
        cmd.env("USER", self.session.user.clone());

        cmd.env(
            "SSHOXY_PUBLIC_KEY",
            self.session
                .public_key
                .clone()
                .map(|k| k.to_string())
                .unwrap_or_default(),
        );
        cmd.env("TERM", self.session.term.clone());

        let mut child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(_) => {
                let _ = session.channel_failure(channel);
                return Ok(());
            }
        };
        let reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(_) => {
                let _ = session.channel_failure(channel);
                return Ok(());
            }
        };
        let writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(_) => {
                let _ = session.channel_failure(channel);
                return Ok(());
            }
        };
        self.session.writers.insert(channel, writer);
        self.session.killers.insert(channel, child.clone_killer());
        self.session.masters.insert(channel, pair.master);
        session.channel_success(channel)?;

        // pump: child stdout -> ssh
        let handle = session.handle();
        let rt = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(e) => {
                log::error!("Failed to get current tokio runtime handle: {}", e);
                return Err(russh::Error::ChannelOpenFailure(
                    russh::ChannelOpenFailure::Other {
                        code: 1,
                        reason: "Couldn't spawn a shell to run a command".to_string(),
                    },
                ));
            }
        };
        tokio::task::spawn_blocking(move || {
            let mut owned = reader;
            let mut buf = [0u8; 8192];
            loop {
                let n = match std::io::Read::read(&mut owned, &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let chunk = match buf.get(..n) {
                    Some(c) => bytes::Bytes::copy_from_slice(c),
                    None => break,
                };
                let h = handle.clone();
                rt.block_on(async move {
                    let _ = h.data(channel, chunk).await;
                });
            }
        });

        // waiter: child exit -> terminate program (exit_status + eof + close, else hang)
        let handle2 = session.handle();
        let rt2 = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => return Ok(()),
        };
        tokio::task::spawn_blocking(move || {
            let status = match child.wait() {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to wait for child process: {}", e);
                    return;
                }
            };
            let code = if status.success() { 0 } else { 1 };
            rt2.block_on(async move {
                let _ = handle2.exit_status_request(channel, code).await;
                let _ = handle2.eof(channel).await;
                let _ = handle2.close(channel).await;
            });
        });
        Ok(())
    }
}

impl server::Handler for Server {
    type Error = russh::Error;

    async fn auth_none(&mut self, user: &str) -> Result<server::Auth, Self::Error> {
        log::debug!("Auth none for user: {}", user);
        if self.no_auth {
            self.session.user = user.to_string();
            return Ok(server::Auth::Accept);
        }
        let mut method_set = MethodSet::empty();
        method_set.push(MethodKind::PublicKey);
        Ok(server::Auth::Reject {
            proceed_with_methods: Some(method_set),
            partial_success: false,
        })
    }

    async fn auth_publickey_offered(
        &mut self,
        _u: &str,
        _k: &russh::keys::ssh_key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        log::debug!("Auth publickey for user: {}, key: {:?}", user, public_key);
        self.session.user = user.to_string();
        self.session.public_key = Some(public_key.clone());
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
        channel: ChannelId,
        data: &[u8],
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let writer = self.session.writers.get_mut(&channel).ok_or_else(|| {
            log::error!("Received data for unknown channel: {:?}", channel);
            russh::Error::WrongChannel
        })?;
        std::io::Write::write_all(writer.as_mut(), data).map_err(|e| {
            log::error!("Failed to write to child stdin: {e}");
            russh::Error::IO(e)
        })?;
        std::io::Write::flush(writer.as_mut()).map_err(|e| {
            log::error!("Failed to flush child stdin: {e}");
            russh::Error::IO(e)
        })?;
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let mut killer = self.session.killers.remove(&channel).ok_or_else(|| {
            log::error!("Close for unknown channel: {:?}", channel);
            russh::Error::WrongChannel
        })?;
        let kill_result = portable_pty::ChildKiller::kill(killer.as_mut()).map_err(|e| {
            log::error!("Failed to kill child: {e}");
            russh::Error::HUP
        });
        self.session.writers.remove(&channel);
        self.session.masters.remove(&channel);
        kill_result
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let mut killer = self.session.killers.remove(&channel).ok_or_else(|| {
            log::error!("EOF for unknown channel: {:?}", channel);
            russh::Error::WrongChannel
        })?;
        let kill_result = portable_pty::ChildKiller::kill(killer.as_mut()).map_err(|e| {
            log::error!("Failed to kill child: {e}");
            russh::Error::HUP
        });
        self.session.writers.remove(&channel);
        self.session.masters.remove(&channel);
        kill_result
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        modes: &[(russh::Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        log::debug!(
            "PTY request: channel: {:?}, term: {}, size: {}x{}, pix: {}x{}, modes: {:?}",
            channel,
            term,
            col_width,
            row_height,
            pix_width,
            pix_height,
            modes
        );
        let term = if term.is_empty() {
            "xterm-256color".to_string()
        } else {
            term.to_string()
        };
        self.session.term = term;
        if let (Ok(cols), Ok(rows), Ok(pixel_width), Ok(pixel_height)) = (
            u16::try_from(col_width),
            u16::try_from(row_height),
            u16::try_from(pix_width),
            u16::try_from(pix_height),
        ) {
            self.session.size = PtySize {
                cols,
                rows,
                pixel_width,
                pixel_height,
            };
        }
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.spawn_shell(channel, session).await
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.spawn_shell(channel, session).await
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let (cols, rows, pixel_width, pixel_height) = match (
            u16::try_from(col_width),
            u16::try_from(row_height),
            u16::try_from(pix_width),
            u16::try_from(pix_height),
        ) {
            (Ok(cols), Ok(rows), Ok(pixel_width), Ok(pixel_height)) => {
                (cols, rows, pixel_width, pixel_height)
            }
            _ => {
                log::error!("Invalid window size: {col_width}x{row_height}");
                return Err(russh::Error::WrongChannel);
            }
        };
        let master = self.session.masters.get(&channel).ok_or_else(|| {
            log::error!("Window change for unknown channel: {:?}", channel);
            russh::Error::WrongChannel
        })?;
        portable_pty::MasterPty::resize(
            master.as_ref(),
            PtySize {
                cols,
                rows,
                pixel_width,
                pixel_height,
            },
        )
        .map_err(|e| {
            log::error!("Failed to resize PTY: {e}");
            russh::Error::HUP
        })?;
        session.channel_success(channel)?;
        Ok(())
    }

    async fn signal(
        &mut self,
        channel: ChannelId,
        signal: russh::Sig,
        _session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        log::debug!("Signal {:?} for channel {:?}", signal, channel);
        let killer = self.session.killers.get_mut(&channel).ok_or_else(|| {
            log::error!("Signal for unknown channel: {:?}", channel);
            russh::Error::WrongChannel
        })?;
        portable_pty::ChildKiller::kill(killer.as_mut()).map_err(|e| {
            log::error!("Failed to kill child on signal: {e}");
            russh::Error::HUP
        })?;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        log::error!("Subsystem request denied: {name}");
        session.channel_failure(channel)?;
        Err(russh::Error::RequestDenied)
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        variable_name: &str,
        _variable_value: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        log::error!("Env request denied: {variable_name}");
        session.channel_failure(channel)?;
        Err(russh::Error::RequestDenied)
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        _port: &mut u32,
        _session: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        log::error!("TCP/IP forward denied: {address}");
        Err(russh::Error::RequestDenied)
    }
}

impl server::Server for Server {
    type Handler = Self;

    fn new_client(&mut self, peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
        if let Some(addr) = peer_addr {
            log::debug!("New client connected from:{}", addr.ip());
        }
        Self {
            welcome_message: self.welcome_message.clone(),
            command: self.command.clone(),
            session: Session::default(),
            no_auth: self.no_auth,
        }
    }

    fn handle_session_error(&mut self, error: <Self::Handler as server::Handler>::Error) {
        log::error!("Session error: {:?}", error);
    }
}
