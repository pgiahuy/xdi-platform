// SPDX-License-Identifier: Apache-2.0
/* engine/llm/http.rs - phần HTTP dùng chung của Gemini và Ollama: dựng client có timeout, mô tả lỗi để ghi log.
Client là reqwest với rustls và bộ mã hóa ring, không dùng OpenSSL của hệ thống: image Docker và binary phát hành
không cần thư viện libssl. Chứng chỉ CA đọc từ hệ thống (gói ca-certificates, image Docker đã cài). */

use std::error::Error as _;
use std::time::Duration;

/// Thời gian tối đa để mở kết nối. Tách khỏi timeout cả lần gọi: máy Ollama tắt hay sai địa chỉ thì báo sau vài giây,
/// không phải chờ hết timeout dài dành cho mô hình chạy chậm.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Dựng client HTTP với timeout cho cả lần gọi (gửi, chờ mô hình, nhận hết body) và cho riêng việc mở kết nối.
/// Trả lỗi dạng chữ khi máy không có chứng chỉ CA nào: người gọi giữ lỗi này và báo khi có câu hỏi, để backend vẫn
/// khởi động được và các phần không cần mô hình vẫn dùng được.
pub fn build_client(
    timeout: Duration,
    connect_timeout: Duration,
) -> Result<reqwest::Client, String> {
    // rustls cần biết dùng bộ mã hóa nào. Đặt một lần cho cả tiến trình; lần sau trả Err vì đã đặt, bỏ qua được.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        // Không theo chuyển hướng: khi chuyển sang máy khác, reqwest chỉ bỏ các header như authorization, còn
        // x-goog-api-key vẫn gửi tiếp, tức khóa Gemini tới tay máy đó. Gemini và Ollama đều không cần chuyển hướng;
        // response 3xx được báo thành lỗi.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(describe)
}

/// Địa chỉ để ghi log: chỉ scheme, host, cổng. Bỏ user:pass (Ollama đặt sau proxy có basic auth) và đường dẫn.
pub fn build_log_url(url: &str) -> String {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return "(invalid URL)".to_string();
    };
    let host = parsed.host_str().unwrap_or("");
    match parsed.port() {
        Some(port) => format!("{}://{host}:{port}", parsed.scheme()),
        None => format!("{}://{host}", parsed.scheme()),
    }
}

/// Mô tả lỗi của reqwest để ghi log: bỏ URL (lỗi reqwest mặc định in URL đầy đủ), nối các lỗi gốc như
/// "Connection refused" vì chính lỗi reqwest chỉ ghi chung chung "error sending request".
pub fn describe(err: reqwest::Error) -> String {
    let err = err.without_url();
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
pub mod test_support {
    /* Server HTTP giả cho test của Gemini và Ollama: trả một response viết sẵn và giữ lại request nhận được.
    Bind cổng 0 để hệ điều hành chọn cổng trống, nhiều test chạy song song không giành cổng của nhau. */

    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use axum::{
        body::Bytes,
        extract::State,
        http::{HeaderMap, StatusCode, Uri},
        response::IntoResponse,
        Router,
    };

    /// Request server giả nhận được.
    #[derive(Clone)]
    pub struct Received {
        pub path: String,
        pub query: Option<String>,
        pub headers: HeaderMap,
        pub body: serde_json::Value,
    }

    #[derive(Clone)]
    struct Fake {
        status: StatusCode,
        body: String,
        delay: Duration,
        received: Arc<Mutex<Option<Received>>>,
    }

    /// Mở server giả, trả địa chỉ gốc (http://127.0.0.1:cổng) và chỗ đọc request nhận được.
    pub async fn spawn(
        status: u16,
        body: impl Into<String>,
        delay: Duration,
    ) -> (String, Arc<Mutex<Option<Received>>>) {
        let received = Arc::new(Mutex::new(None));
        let fake = Fake {
            status: StatusCode::from_u16(status).expect("valid status"),
            body: body.into(),
            delay,
            received: received.clone(),
        };
        let app = Router::new().fallback(handle).with_state(fake);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        (format!("http://{addr}"), received)
    }

    async fn handle(
        State(fake): State<Fake>,
        uri: Uri,
        headers: HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        let body = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        *fake.received.lock().expect("lock") = Some(Received {
            path: uri.path().to_string(),
            query: uri.query().map(str::to_string),
            headers,
            body,
        });
        tokio::time::sleep(fake.delay).await;
        (
            fake.status,
            [("content-type", "application/json")],
            fake.body,
        )
    }

    /// Server giả trả 307 chuyển sang `location`.
    pub async fn spawn_redirect(location: String) -> String {
        let app = Router::new().fallback(move || async move {
            (
                StatusCode::TEMPORARY_REDIRECT,
                [("location", location.clone())],
            )
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        format!("http://{addr}")
    }

    /// Địa chỉ nhận kết nối nhưng không bao giờ bắt tay xong, như máy có tường lửa bỏ gói (DROP): listener không gọi
    /// accept và hàng chờ đã đầy, nên Linux bỏ gói SYN mới. Giữ giá trị trả về sống trong suốt test.
    pub struct Unresponsive {
        pub url: String,
        _listener: tokio::net::TcpListener,
        _queued: Vec<tokio::net::TcpStream>,
    }

    pub async fn unresponsive() -> Unresponsive {
        let socket = tokio::net::TcpSocket::new_v4().expect("socket");
        socket
            .bind("127.0.0.1:0".parse().expect("addr"))
            .expect("bind");
        let listener = socket.listen(0).expect("listen");
        let addr = listener.local_addr().expect("local addr");
        // Lấp hàng chờ: mở kết nối tới khi một lần mở không xong trong 200 ms.
        let mut queued = Vec::new();
        loop {
            let attempt = tokio::time::timeout(
                Duration::from_millis(200),
                tokio::net::TcpStream::connect(addr),
            );
            match attempt.await {
                Ok(Ok(stream)) => queued.push(stream),
                _ => break,
            }
            assert!(queued.len() < 64, "accept queue never filled");
        }
        Unresponsive {
            url: format!("http://{addr}"),
            _listener: listener,
            _queued: queued,
        }
    }

    /// Địa chỉ chắc chắn không có ai nghe: bind một cổng rồi đóng ngay.
    pub async fn closed_url() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        format!("http://{addr}")
    }

    /// Bộ đệm nhận log của test.
    #[derive(Clone, Default)]
    pub struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl LogBuffer {
        pub fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().expect("lock")).into_owned()
        }
    }

    impl Write for LogBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Ghi mọi log tới mức trace của luồng hiện tại vào bộ đệm cho tới khi guard bị hủy, gồm cả log của reqwest, hyper
    /// và rustls (các crate này ghi bằng crate log, LogTracer chuyển sang tracing). Test tokio mặc định chạy trên một
    /// luồng, nên log của cả lời gọi lẫn server giả đều vào đây.
    pub fn capture_logs() -> (tracing::subscriber::DefaultGuard, LogBuffer) {
        // LogTracer đặt cho cả tiến trình; test khác đã đặt rồi thì trả Err, bỏ qua được.
        let _ = tracing_log::LogTracer::init();
        let buffer = LogBuffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        (tracing::subscriber::set_default(subscriber), buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_url_drops_credentials_and_path() {
        assert_eq!(
            build_log_url("https://user:secret@ollama.lan:8443/proxy/"),
            "https://ollama.lan:8443"
        );
        assert_eq!(
            build_log_url("http://localhost:11434"),
            "http://localhost:11434"
        );
        assert_eq!(build_log_url("not a url"), "(invalid URL)");
    }
}
