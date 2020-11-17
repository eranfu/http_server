use std::future::Future;
use std::time::Duration;

use async_listen::backpressure::Token;
use async_listen::ListenExt;
use async_std::fs;
use async_std::io::BufReader;
use async_std::net::{TcpListener, TcpStream};
use async_std::task;
use async_std::task::JoinHandle;
use futures::{AsyncBufReadExt, AsyncWriteExt, StreamExt};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[async_std::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:7878").await?;
    listener
        .incoming()
        .log_warnings(log_warnings)
        .handle_errors(Duration::from_millis(500))
        .backpressure(100)
        .for_each(|(token, stream)| async move {
            spawn_and_log_error(handle_connection(token, stream));
        })
        .await;

    Ok(())
}

async fn await_and_log_error(fut: impl Future<Output = Result<()>>) {
    if let Err(e) = fut.await {
        eprintln!("{}", e);
    }
}

fn spawn_and_log_error(fut: impl 'static + Future<Output = Result<()>> + Send) -> JoinHandle<()> {
    task::spawn(await_and_log_error(fut))
}

fn log_warnings(error: &std::io::Error) {
    eprintln!("Error: {}. Listener paused for 0.5s", error);
}

async fn handle_connection(_token: Token, mut stream: TcpStream) -> Result<()> {
    let stream_reader = BufReader::new(&stream);
    let mut lines = stream_reader.lines();
    let request_line = match lines.next().await {
        None => Err("peer disconnected immediately.")?,
        Some(line) => line?,
    };

    const INDEX_PATTERN: &str = "GET / HTTP/1.1";
    const SLEEP_PATTERN: &str = "GET /sleep HTTP/1.1";

    // Respond with greetings or a 404,
    // depending on the data in the request
    let (status_line, res_file_path) = if request_line.eq(INDEX_PATTERN) {
        ("HTTP/1.1 200 OK", "resources/web_root/hello.html")
    } else if request_line.eq(SLEEP_PATTERN) {
        task::sleep(Duration::from_secs(5)).await;
        ("HTTP/1.1 200 OK", "resources/web_root/hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "resources/web_root/404.html")
    };
    let contents = fs::read_to_string(res_file_path).await?;
    stream.write_all(status_line.as_bytes()).await?;
    stream.write_all("\r\n\r\n".as_bytes()).await?;
    stream.write_all(contents.as_bytes()).await?;

    Ok(())
}
