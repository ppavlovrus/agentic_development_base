//! Graceful shutdown signal handling for binaries.

/// Resolves on SIGINT (Ctrl-C) or SIGTERM (container stop).
pub async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        // `expect` here is a deliberate exception to the "no expect in
        // libraries" rule: signal registration cannot realistically fail,
        // and there is nothing meaningful to fall back to.
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("installing the SIGTERM handler cannot fail");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
    tracing::info!("shutdown signal received");
}
