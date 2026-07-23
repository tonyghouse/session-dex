use std::{
    env,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PROTOCOL_MAGIC: &[u8; 8] = b"SDEXIPC1";
const PROTOCOL_VERSION: u32 = 1;
const ACKNOWLEDGED: u8 = 1;
const MAX_TOKEN_BYTES: usize = 128;
const MAX_ENDPOINT_BYTES: u64 = 1024;
const FORWARD_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const RETRY_DELAY: Duration = Duration::from_millis(25);

pub enum InstanceMessage {
    Launch,
    Fatal(String),
}

pub struct InstanceGuard {
    _file: File,
}

pub struct InstanceOwner {
    pub guard: InstanceGuard,
    pub receiver: Receiver<InstanceMessage>,
}

pub enum InstanceClaim {
    Primary(InstanceOwner),
    Forwarded,
}

struct Endpoint {
    port: u16,
    token: String,
}

pub fn claim_or_forward() -> Result<InstanceClaim, String> {
    let instance_dir = instance_directory()?;
    std::fs::create_dir_all(&instance_dir).map_err(|error| {
        format!(
            "Failed to create instance directory {}: {error}",
            instance_dir.display()
        )
    })?;
    secure_instance_directory(&instance_dir)?;

    let lock_path = instance_dir.join("instance.lock");
    let mut file = open_lock_file(&lock_path)?;
    let started = Instant::now();

    loop {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return start_primary(file),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => {
                return Err(format!(
                    "Failed to claim the SessionDex process lock at {}: {error}",
                    lock_path.display()
                ));
            }
        }

        if let Ok(endpoint) = read_endpoint(&mut file) {
            if forward_launch(&endpoint).is_ok() {
                return Ok(InstanceClaim::Forwarded);
            }
        }

        if started.elapsed() >= FORWARD_TIMEOUT {
            return Err(format!(
                "Another SessionDex process owns {}, but did not acknowledge this launch within {} seconds.",
                lock_path.display(),
                FORWARD_TIMEOUT.as_secs()
            ));
        }

        thread::sleep(RETRY_DELAY);
    }
}

fn start_primary(mut file: File) -> Result<InstanceClaim, String> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("Failed to create the local SessionDex IPC listener: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Failed to inspect the local SessionDex IPC listener: {error}"))?
        .port();
    let token = instance_token();

    write_endpoint(
        &mut file,
        &Endpoint {
            port,
            token: token.clone(),
        },
    )
    .map_err(|error| format!("Failed to publish the local SessionDex IPC endpoint: {error}"))?;

    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("sessiondex-instance-ipc".to_string())
        .spawn(move || listen_for_launches(listener, token, sender))
        .map_err(|error| format!("Failed to start the local SessionDex IPC listener: {error}"))?;

    Ok(InstanceClaim::Primary(InstanceOwner {
        guard: InstanceGuard { _file: file },
        receiver,
    }))
}

fn listen_for_launches(listener: TcpListener, token: String, sender: Sender<InstanceMessage>) {
    loop {
        let (mut stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = sender.send(InstanceMessage::Fatal(format!(
                    "The local SessionDex IPC listener failed: {error}"
                )));
                return;
            }
        };

        if read_launch(&mut stream, &token).is_err() {
            continue;
        }

        if sender.send(InstanceMessage::Launch).is_err() {
            return;
        }

        let _ = stream.write_all(&[ACKNOWLEDGED]);
    }
}

fn forward_launch(endpoint: &Endpoint) -> io::Result<()> {
    let address: std::net::SocketAddr =
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, endpoint.port).into();
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)?;
    configure_stream(&stream)?;
    stream.write_all(PROTOCOL_MAGIC)?;
    stream.write_all(&PROTOCOL_VERSION.to_be_bytes())?;
    write_token(&mut stream, &endpoint.token)?;
    stream.flush()?;

    let mut acknowledgement = [0];
    stream.read_exact(&mut acknowledgement)?;

    if acknowledgement[0] != ACKNOWLEDGED {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SessionDex did not acknowledge the forwarded launch",
        ));
    }

    Ok(())
}

fn read_launch(stream: &mut TcpStream, expected_token: &str) -> io::Result<()> {
    configure_stream(stream)?;
    let mut magic = [0; PROTOCOL_MAGIC.len()];
    stream.read_exact(&mut magic)?;

    if &magic != PROTOCOL_MAGIC || read_u32(stream)? != PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported SessionDex IPC protocol",
        ));
    }

    if read_token(stream)? != expected_token {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "invalid SessionDex IPC token",
        ));
    }

    Ok(())
}

fn configure_stream(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))
}

fn write_token(writer: &mut impl Write, value: &str) -> io::Result<()> {
    let bytes = value.as_bytes();
    if bytes.len() > MAX_TOKEN_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SessionDex IPC token exceeds the size limit",
        ));
    }

    writer.write_all(&(bytes.len() as u32).to_be_bytes())?;
    writer.write_all(bytes)
}

fn read_token(reader: &mut impl Read) -> io::Result<String> {
    let length = read_u32(reader)? as usize;
    if length > MAX_TOKEN_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SessionDex IPC token exceeds the size limit",
        ));
    }

    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; size_of::<u32>()];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

fn write_endpoint(file: &mut File, endpoint: &Endpoint) -> io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    write!(
        file,
        "{}\n{}\n{}\n",
        PROTOCOL_VERSION, endpoint.port, endpoint.token
    )?;
    file.sync_data()
}

fn read_endpoint(file: &mut File) -> io::Result<Endpoint> {
    file.seek(SeekFrom::Start(0))?;
    let mut contents = String::new();
    file.take(MAX_ENDPOINT_BYTES)
        .read_to_string(&mut contents)?;
    let mut lines = contents.lines();
    let version = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing endpoint version"))?
        .parse::<u32>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    if version != PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported endpoint version",
        ));
    }

    let port = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing endpoint port"))?
        .parse::<u16>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let token = lines
        .next()
        .filter(|token| !token.is_empty() && token.len() <= MAX_TOKEN_BYTES)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing endpoint token"))?
        .to_string();

    Ok(Endpoint { port, token })
}

fn open_lock_file(path: &PathBuf) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let file = options
        .open(path)
        .map_err(|error| format!("Failed to open {}: {error}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Failed to secure {}: {error}", path.display()))?;
    }

    Ok(file)
}

fn secure_instance_directory(path: &PathBuf) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Failed to secure {}: {error}", path.display()))?;
    }

    Ok(())
}

fn instance_directory() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("io.sessiondex.desktop"))
            .ok_or_else(|| "LOCALAPPDATA is not set; refusing to start SessionDex.".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library/Application Support/io.sessiondex.desktop"))
            .ok_or_else(|| "HOME is not set; refusing to start SessionDex.".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".local/share/io.sessiondex.desktop"))
            .ok_or_else(|| "HOME is not set; refusing to start SessionDex.".to_string());
    }

    #[allow(unreachable_code)]
    Err("SessionDex single-process enforcement is unsupported on this platform.".to_string())
}

fn instance_token() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    format!("{:x}-{timestamp:x}", std::process::id())
}
