use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use flate2::{write::GzEncoder, Compression};
use lex_net::{
    HeaderName, HeaderValue, LexUrl, NavigationRequest, NetworkClient, NetworkError,
};

fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn serve<F>(requests: usize, handler: F) -> (String, thread::JoinHandle<()>)
where
    F: Fn(usize, &str) -> Vec<u8> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        for (index, incoming) in listener.incoming().take(requests).enumerate() {
            let mut stream = incoming.unwrap();
            let request = read_request(&mut stream);
            stream.write_all(&handler(index, &request)).unwrap();
        }
    });
    (format!("http://{address}"), handle)
}

fn response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut result = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n", body.len()).into_bytes();
    for (name, value) in headers {
        result.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    result.extend_from_slice(b"Connection: close\r\n\r\n");
    result.extend_from_slice(body);
    result
}

#[test]
fn sends_headers_and_returns_response_metadata() {
    let captured = Arc::new(Mutex::new(String::new()));
    let request_copy = Arc::clone(&captured);
    let (base, server) = serve(1, move |_, request| {
        *request_copy.lock().unwrap() = request.into();
        response("200 OK", &[("Content-Type", "text/plain")], b"hello lex")
    });
    let client = NetworkClient::new().unwrap();
    let mut request = NavigationRequest::get(LexUrl::parse(&base).unwrap());
    request.headers.insert(
        HeaderName::from_static("x-lex-test"),
        HeaderValue::from_static("present"),
    );
    let result = client.navigate(&request).unwrap();
    server.join().unwrap();
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"hello lex");
    assert!(captured.lock().unwrap().to_ascii_lowercase().contains("x-lex-test: present"));
}

#[test]
fn follows_relative_redirect_and_records_chain() {
    let (base, server) = serve(2, |index, _| {
        if index == 0 {
            response("302 Found", &[("Location", "/final")], b"")
        } else {
            response("200 OK", &[], b"arrived")
        }
    });
    let result = NetworkClient::new()
        .unwrap()
        .navigate(&NavigationRequest::get(LexUrl::parse(&format!("{base}/start")).unwrap()))
        .unwrap();
    server.join().unwrap();
    assert_eq!(result.body, b"arrived");
    assert_eq!(result.redirect_chain.len(), 1);
    assert!(result.final_url.as_str().ends_with("/final"));
}

#[test]
fn detects_redirect_loop() {
    let (base, server) = serve(2, |index, _| {
        let location = if index == 0 { "/two" } else { "/one" };
        response("302 Found", &[("Location", location)], b"")
    });
    let error = NetworkClient::new()
        .unwrap()
        .navigate(&NavigationRequest::get(LexUrl::parse(&format!("{base}/one")).unwrap()))
        .unwrap_err();
    server.join().unwrap();
    assert!(matches!(error, NetworkError::RedirectLoop(_)));
}

#[test]
fn decodes_gzip_response() {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(b"compressed document").unwrap();
    let compressed = encoder.finish().unwrap();
    let (base, server) = serve(1, move |_, _| {
        response("200 OK", &[("Content-Encoding", "gzip")], &compressed)
    });
    let result = NetworkClient::new()
        .unwrap()
        .navigate(&NavigationRequest::get(LexUrl::parse(&base).unwrap()))
        .unwrap();
    server.join().unwrap();
    assert_eq!(result.body, b"compressed document");
}

#[test]
fn enforces_decoded_response_limit() {
    let (base, server) = serve(1, |_, _| response("200 OK", &[], b"0123456789"));
    let mut request = NavigationRequest::get(LexUrl::parse(&base).unwrap());
    request.max_response_bytes = 5;
    let error = NetworkClient::new().unwrap().navigate(&request).unwrap_err();
    server.join().unwrap();
    assert!(matches!(
        error,
        NetworkError::OversizedResponse { .. } | NetworkError::ResponseLimitExceeded { .. }
    ));
}

#[test]
fn reuses_fresh_cache_entry_without_second_request() {
    let (base, server) = serve(1, |_, _| {
        response("200 OK", &[("Cache-Control", "public, max-age=60")], b"once")
    });
    let client = NetworkClient::new().unwrap();
    let request = NavigationRequest::get(LexUrl::parse(&base).unwrap());
    assert!(!client.navigate(&request).unwrap().from_cache);
    server.join().unwrap();
    let cached = client.navigate(&request).unwrap();
    assert!(cached.from_cache);
    assert_eq!(cached.body, b"once");
}

#[test]
fn reports_connection_failure_without_panicking() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let request = NavigationRequest::get(LexUrl::parse(&format!("http://{address}")).unwrap());
    assert!(matches!(
        NetworkClient::new().unwrap().navigate(&request),
        Err(NetworkError::Transport(_))
    ));
}
