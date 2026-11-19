//! O que o D-Bus da sessão diz de um player: o PID do dono do nome e o
//! `DesktopEntry` do MPRIS, atrás do trait [`Bus`].

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::timeout;
use zbus::Connection;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::proxy::CacheProperties;

/// Um futuro em caixa, para o trait [`Bus`] ser usado como objeto.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// O prazo padrão de cada chamada ao D-Bus.
pub const DBUS_TIMEOUT: Duration = Duration::from_secs(1);

/// O que se sabe do player `name`; o que não deu para saber fica `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    /// O nome D-Bus do player (`org.mpris.MediaPlayer2.*`).
    pub name: String,
    /// O PID do dono do nome.
    pub owner_pid: Option<u32>,
    /// A propriedade `DesktopEntry` do MPRIS.
    pub desktop_entry: Option<String>,
}

/// O barramento que identifica os players.
pub trait Bus: Send + Sync + 'static {
    /// O que o barramento diz de `name`; nunca falha, só deixa campos `None`.
    fn identify<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Identity>;
}

/// Como o [`SessionBus`] abre a conexão.
pub type Connect = Box<dyn Fn() -> BoxFuture<'static, zbus::Result<Connection>> + Send + Sync>;

/// O D-Bus da sessão do usuário, pelo `zbus`.
pub struct SessionBus {
    timeout: Duration,
    connect: Connect,
    /// A conexão guardada; volta a `None` depois de um erro de E/S ou do
    /// prazo do barramento estourado, para a próxima chamada refazer.
    conn: Mutex<Option<Connection>>,
    /// A conexão que não respondeu o PID no prazo, marcada sem `await` (um
    /// prazo de fora pode largar a chamada antes do de dentro): a próxima
    /// [`SessionBus::connection`] a esquece se ainda for a guardada.
    stale: std::sync::Mutex<Option<ConnId>>,
}

/// O que identifica uma conexão.
type ConnId = (Option<String>, String);

fn conn_id(conn: &Connection) -> ConnId {
    (
        conn.unique_name().map(|n| n.to_string()),
        conn.server_guid().to_string(),
    )
}

/// Marca a conexão como sem resposta ao cair, se não for desarmado: vale
/// para o prazo de dentro e para o futuro largado por um prazo de fora.
struct StaleOnDrop<'a> {
    stale: &'a std::sync::Mutex<Option<ConnId>>,
    conn: Option<ConnId>,
}

impl Drop for StaleOnDrop<'_> {
    fn drop(&mut self) {
        if let Some(id) = self.conn.take() {
            *self.stale.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        }
    }
}

impl SessionBus {
    /// Um barramento com `timeout` em cada chamada (a conexão, o PID e o
    /// `DesktopEntry`). Não conecta até o primeiro [`Bus::identify`].
    pub fn new(timeout: Duration) -> SessionBus {
        SessionBus::with_connect(timeout, Box::new(|| Box::pin(Connection::session())))
    }

    /// O [`SessionBus::new`] com a conexão aberta por `connect` (nos testes,
    /// uma conexão ponto a ponto).
    pub fn with_connect(timeout: Duration, connect: Connect) -> SessionBus {
        SessionBus {
            timeout,
            connect,
            conn: Mutex::new(None),
            stale: std::sync::Mutex::new(None),
        }
    }

    async fn connection(&self) -> Option<Connection> {
        let mut conn = self.conn.lock().await;
        let stale = self.stale.lock().unwrap_or_else(|e| e.into_inner()).take();
        if stale.is_some() && conn.as_ref().map(conn_id) == stale {
            *conn = None;
        }
        if let Some(conn) = conn.as_ref() {
            return Some(conn.clone());
        }
        let fresh = timeout(self.timeout, (self.connect)()).await.ok()?.ok()?;
        *conn = Some(fresh.clone());
        Some(fresh)
    }

    /// Esquece a conexão se o erro for de E/S (o barramento caiu).
    async fn forget_on_io(&self, err: &zbus::Error) {
        if matches!(err, zbus::Error::InputOutput(_)) {
            *self.conn.lock().await = None;
        }
    }

    /// O PID do dono de `name`, pelo próprio barramento. O prazo estourado
    /// aqui é do barramento (travado ou com a conexão perdida sem erro de
    /// E/S): a conexão é esquecida. O do `DesktopEntry` não, porque quem
    /// responde é o player, e um player travado não diz nada da conexão.
    async fn owner_pid(&self, conn: &Connection, name: &BusName<'_>) -> Option<u32> {
        let call = async {
            let proxy = DBusProxy::new(conn).await?;
            proxy
                .get_connection_unix_process_id(name.clone())
                .await
                .map_err(zbus::Error::from)
        };
        // Armada até a resposta: o prazo estourado aqui ou o futuro largado
        // por um prazo de fora marcam a conexão.
        let mut armed = StaleOnDrop {
            stale: &self.stale,
            conn: Some(conn_id(conn)),
        };
        let result = timeout(self.timeout, call).await.ok()?;
        armed.conn = None;
        match result {
            Ok(pid) => Some(pid),
            Err(err) => {
                self.forget_on_io(&err).await;
                None
            }
        }
    }

    async fn desktop_entry(&self, conn: &Connection, name: &BusName<'_>) -> Option<String> {
        let call = async {
            let proxy: zbus::Proxy<'_> = zbus::proxy::Builder::new(conn)
                .destination(name.clone())?
                .path("/org/mpris/MediaPlayer2")?
                .interface("org.mpris.MediaPlayer2")?
                .cache_properties(CacheProperties::No)
                .build()
                .await?;
            proxy.get_property::<String>("DesktopEntry").await
        };
        match timeout(self.timeout, call).await.ok()? {
            Ok(entry) => Some(entry),
            Err(err) => {
                self.forget_on_io(&err).await;
                None
            }
        }
    }
}

impl Bus for SessionBus {
    fn identify<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Identity> {
        Box::pin(async move {
            let mut id = Identity {
                name: name.to_string(),
                ..Identity::default()
            };
            let Ok(bus_name) = BusName::try_from(name) else {
                return id;
            };
            let Some(conn) = self.connection().await else {
                return id;
            };
            // Juntos, para um prazo de fora cobrir o total sem perder o PID.
            (id.owner_pid, id.desktop_entry) = tokio::join!(
                self.owner_pid(&conn, &bus_name),
                self.desktop_entry(&conn, &bus_name),
            );
            id
        })
    }
}
