//! 跨平台 IPC 传输抽象 — Windows Named Pipe / Unix Domain Socket

use tokio::io::{AsyncRead, AsyncWrite};

use super::named_pipe_path;

/// IPC 流：双向异步读写
pub enum IpcStream {
    #[cfg(windows)]
    NamedPipeServer(tokio::net::windows::named_pipe::NamedPipeServer),
    #[cfg(windows)]
    NamedPipeClient(tokio::net::windows::named_pipe::NamedPipeClient),
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
}

impl AsyncRead for IpcStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            #[cfg(windows)]
            IpcStream::NamedPipeServer(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            #[cfg(windows)]
            IpcStream::NamedPipeClient(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            #[cfg(unix)]
            IpcStream::Unix(s) => std::pin::Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for IpcStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            #[cfg(windows)]
            IpcStream::NamedPipeServer(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            #[cfg(windows)]
            IpcStream::NamedPipeClient(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            #[cfg(unix)]
            IpcStream::Unix(s) => std::pin::Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            #[cfg(windows)]
            IpcStream::NamedPipeServer(s) => std::pin::Pin::new(s).poll_flush(cx),
            #[cfg(windows)]
            IpcStream::NamedPipeClient(s) => std::pin::Pin::new(s).poll_flush(cx),
            #[cfg(unix)]
            IpcStream::Unix(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            #[cfg(windows)]
            IpcStream::NamedPipeServer(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            #[cfg(windows)]
            IpcStream::NamedPipeClient(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            #[cfg(unix)]
            IpcStream::Unix(s) => std::pin::Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// 创建 IPC 服务端监听器
pub async fn create_server() -> std::io::Result<IpcServer> {
    let path = named_pipe_path::full_path();
    IpcServer::bind(&path).await
}

/// 创建 IPC 客户端连接
pub async fn connect_client() -> std::io::Result<IpcStream> {
    let path = named_pipe_path::full_path();
    IpcClient::connect(&path).await
}

/// IPC 服务端
pub struct IpcServer {
    #[cfg(windows)]
    pipe_name: String,
    #[cfg(windows)]
    listener: tokio::net::windows::named_pipe::NamedPipeServer,
    #[cfg(unix)]
    socket_path: std::path::PathBuf,
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
}

impl IpcServer {
    pub async fn bind(path: &str) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            let listener = tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(true)
                .create(path)?;
            Ok(Self {
                pipe_name: path.to_string(),
                listener,
            })
        }
        #[cfg(unix)]
        {
            let socket_path = std::path::PathBuf::from(path);
            // 确保父目录存在（如 $XDG_RUNTIME_DIR/codeorbit/）
            if let Some(parent) = socket_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // 清理旧 socket 文件
            let _ = std::fs::remove_file(&socket_path);
            let listener = tokio::net::UnixListener::bind(&socket_path)?;
            Ok(Self {
                socket_path,
                listener,
            })
        }
    }

    /// 接受下一个连接
    pub async fn accept(&mut self) -> std::io::Result<IpcStream> {
        #[cfg(windows)]
        {
            self.listener.connect().await?;
            // 交换：当前 listener 变成 accepted stream，再创建新 listener
            let accepted = std::mem::replace(
                &mut self.listener,
                tokio::net::windows::named_pipe::ServerOptions::new().create(&self.pipe_name)?,
            );
            Ok(IpcStream::NamedPipeServer(accepted))
        }
        #[cfg(unix)]
        {
            let (stream, _addr) = self.listener.accept().await?;
            Ok(IpcStream::Unix(stream))
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// IPC 客户端
pub struct IpcClient;

impl IpcClient {
    pub async fn connect(path: &str) -> std::io::Result<IpcStream> {
        #[cfg(windows)]
        {
            let client = tokio::net::windows::named_pipe::ClientOptions::new().open(path)?;
            Ok(IpcStream::NamedPipeClient(client))
        }
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(path).await?;
            Ok(IpcStream::Unix(stream))
        }
    }
}
