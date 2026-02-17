#![allow(dead_code)]

pub struct MockEnclaveServer {
    pub base_url: String,
    handle: tokio::task::JoinHandle<()>,
}

impl MockEnclaveServer {
    pub async fn start(app: axum::Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock enclave listener should bind");
        let bind_addr = listener
            .local_addr()
            .expect("mock enclave listener local address should exist");

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock enclave server should run");
        });

        Self {
            base_url: format!("http://{bind_addr}"),
            handle,
        }
    }
}

impl Drop for MockEnclaveServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
