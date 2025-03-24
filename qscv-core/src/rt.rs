//! runtime features
//!
//! all functions always available regardless of whether runtime feature is enabled,
//! but calling one without runtime feature will panic
use std::time::Duration;

macro_rules! rt_tokio {
    {$($tt:tt)*} => {
        #[cfg(feature = "tokio")]
        { $($tt)* }

        #[cfg(not(feature = "tokio"))]
        panic!("runtime disabled")
    };
}

// ===== time =====

pub async fn timeout<F: Future>(duration: Duration, f: F) -> Result<F::Output, TimeOutError> {
    rt_tokio! {
        tokio::time::timeout(duration, f).await.map_err(|_|TimeOutError)
    }
}

pub async fn sleep(duration: Duration) {
    rt_tokio! {
        tokio::time::sleep(duration).await
    }
}

#[derive(Debug, thiserror::Error)]
#[error("operation timed out")]
pub struct TimeOutError;

// ===== task =====

pub fn spawn<F>(f: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    rt_tokio! {
        JoinHandle::Tokio(tokio::task::spawn(f))
    }
}

pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    rt_tokio! {
        JoinHandle::Tokio(tokio::task::spawn_blocking(f))
    }
}

pub async fn yield_now() {
    rt_tokio! {
        tokio::task::yield_now().await
    }
}

#[derive(Debug)]
pub enum JoinHandle<T> {
    #[cfg(feature = "tokio")]
    Tokio(tokio::task::JoinHandle<T>),
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match &mut *self {
            JoinHandle::Tokio(handle) => std::pin::Pin::new(handle)
                .poll(cx)
                .map(|res| res.expect("spawned task panicked")),
        }
    }
}


