//! A leitura do conteúdo de uma cópia pelo pipe, com prazo e limite, a
//! escrita para quem cola, com prazo, e o caminho do socket do Wayland.

use std::io::{Error, ErrorKind, PipeReader, Read};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

/// O texto do `clipboard.status` quando não há exibição para observar.
pub const NO_DISPLAY: &str = "Sem WAYLAND_DISPLAY: o lucernad não sabe qual sessão observar.";

const CHUNK: usize = 64 << 10;

/// Por que a leitura de uma cópia falhou.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    /// O conteúdo passou do limite; o resto nem foi lido.
    TooBig,
    /// Quem copiou não fechou o pipe até o prazo.
    TimedOut,
    /// Outro erro de leitura.
    Io(String),
}

/// Lê o pipe até o fim (quem escreve fecha), sem passar de `max` bytes nem
/// do `deadline`. Bloqueia a thread: chame fora do laço.
pub fn read_pipe(
    mut reader: PipeReader,
    max: usize,
    deadline: Instant,
) -> Result<Vec<u8>, ReadError> {
    let mut data = Vec::new();
    let mut chunk = vec![0; CHUNK];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left == Duration::ZERO {
            return Err(ReadError::TimedOut);
        }
        let timeout = Timespec::try_from(left).map_err(|e| ReadError::Io(e.to_string()))?;
        let mut fds = [PollFd::new(&reader, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => continue,
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(e) => return Err(ReadError::Io(e.to_string())),
        }
        // Lê no máximo um byte além do limite: basta para saber que passou.
        let want = chunk.len().min(max + 1 - data.len());
        match reader.read(&mut chunk[..want]) {
            Ok(0) => return Ok(data),
            Ok(n) => {
                data.extend_from_slice(&chunk[..n]);
                if data.len() > max {
                    return Err(ReadError::TooBig);
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(ReadError::Io(e.to_string())),
        }
    }
}

/// Escreve `data` inteiro em `fd` (o pipe de quem cola), sem passar do
/// `deadline`: o fd fica não bloqueante durante a escrita e cada pedaço
/// espera o `POLLOUT` com o tempo que falta. Quem não lê dá
/// `ErrorKind::TimedOut`; quem fecha antes do fim, o erro de pipe quebrado.
/// Bloqueia a thread: chame fora do laço.
pub fn write_pipe(fd: impl AsFd, data: &[u8], deadline: Instant) -> std::io::Result<()> {
    // O fd veio de quem cola, com a mesma descrição de arquivo: o modo é
    // dela, e volta como estava no fim, deu certo ou não.
    let flags = fcntl_getfl(&fd)?;
    fcntl_setfl(&fd, flags | OFlags::NONBLOCK)?;
    let sent = write_all(&fd, data, deadline);
    let restored = fcntl_setfl(&fd, flags);
    sent?;
    Ok(restored?)
}

/// O laço do [`write_pipe`], com o fd já não bloqueante.
fn write_all(fd: impl AsFd, data: &[u8], deadline: Instant) -> std::io::Result<()> {
    let mut rest = data;
    while !rest.is_empty() {
        match rustix::io::write(&fd, rest) {
            Ok(n) => {
                rest = &rest[n..];
                continue;
            }
            Err(rustix::io::Errno::AGAIN) | Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left == Duration::ZERO {
            return Err(Error::new(
                ErrorKind::TimedOut,
                "quem cola não leu no prazo",
            ));
        }
        let timeout = Timespec::try_from(left).map_err(Error::other)?;
        let mut fds = [PollFd::new(&fd, PollFlags::OUT)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// O caminho do socket de uma exibição do Wayland: absoluto vale como está;
/// relativo fica dentro do `runtime_dir` (o `XDG_RUNTIME_DIR`).
pub fn socket_path(display: &str, runtime_dir: Option<&Path>) -> Result<PathBuf, String> {
    if display.is_empty() {
        return Err(NO_DISPLAY.to_string());
    }
    let display = Path::new(display);
    if display.is_absolute() {
        return Ok(display.to_path_buf());
    }
    match runtime_dir {
        Some(dir) => Ok(dir.join(display)),
        None => Err("Sem XDG_RUNTIME_DIR para achar o Wayland.".to_string()),
    }
}
