use crate::error::Result;

/// an either `TcpStream` or `Socket`, which implement
/// `AsyncRead` and `AsyncWrite` transparently
#[derive(Debug)]
pub struct Socket {
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    #[cfg(feature = "tokio")]
    TokioTcp(tokio::net::TcpStream),
    #[cfg(all(feature = "tokio", unix))]
    TokioUnixSocket(tokio::net::UnixStream),
}

impl Socket {
    pub async fn connect_tcp(host: &str, port: u16) -> Result<Socket> {
        #[cfg(feature = "tokio")]
        {
            let socket = tokio::net::TcpStream::connect((host,port)).await?;
            socket.set_nodelay(true)?;
            Ok(Socket { kind: Kind::TokioTcp(socket) })
        }

        #[cfg(not(feature = "tokio"))]
        {
            let _ = (host,port);
            panic!("runtime disabled")
        }
    }

    pub async fn connect_socket(path: &str) -> Result<Socket> {
        #[cfg(feature = "tokio")]
        {
            let socket = tokio::net::UnixStream::connect(path).await?;
            Ok(Socket { kind: Kind::TokioUnixSocket(socket) })
        }

        #[cfg(not(feature = "tokio"))]
        {
            let _ = path;
            panic!("runtime disabled")
        }
    }

    pub async fn read_buf<'a, B>(&'a mut self, buf: &'a mut B) -> Result<usize>
    where
        B: bytes::BufMut + ?Sized,
    {
        #[cfg(feature = "tokio")]
        {
            Ok(tokio::io::AsyncReadExt::read_buf(self, buf).await?)
        }

        #[cfg(not(feature = "tokio"))]
        {
            let _ = buf;
            panic!("runtime disabled")
        }
    }

    pub async fn write_buf<'a, B>(&'a mut self, buf: &'a mut B) -> Result<()>
    where
        B: bytes::Buf,
    {
        #[cfg(feature = "tokio")]
        {
            Ok(tokio::io::AsyncWriteExt::write_all_buf(self, buf).await?)
        }

        #[cfg(not(feature = "tokio"))]
        {
            let _ = buf;
            panic!("runtime disabled")
        }
    }
}

#[cfg(feature = "tokio")]
impl tokio::io::AsyncRead for Socket {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use std::pin::Pin;
        match &mut self.kind {
            Kind::TokioTcp(t) => Pin::new(t).poll_read(cx, buf),
            Kind::TokioUnixSocket(u) => Pin::new(u).poll_read(cx, buf),
        }
    }
}

#[cfg(feature = "tokio")]
impl tokio::io::AsyncWrite for Socket {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        use std::pin::Pin;
        match &mut self.kind {
            Kind::TokioTcp(t) => Pin::new(t).poll_write(cx, buf),
            Kind::TokioUnixSocket(u) => Pin::new(u).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        use std::pin::Pin;
        match &mut self.kind {
            Kind::TokioTcp(t) => Pin::new(t).poll_flush(cx),
            Kind::TokioUnixSocket(u) => Pin::new(u).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        use std::pin::Pin;
        match &mut self.kind {
            Kind::TokioTcp(t) => Pin::new(t).poll_shutdown(cx),
            Kind::TokioUnixSocket(u) => Pin::new(u).poll_shutdown(cx),
        }
    }
}

