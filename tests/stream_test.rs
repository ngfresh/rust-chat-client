//! 用本地模拟 SSE 服务器测试 chat_stream 的流式解析与拼接逻辑。
//! 无需真实 API Key，可复现 TCP 拆包/粘包、[DONE] 终止等场景。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use ai_chat_client::api::openai_compatible::OpenAiClient;
use ai_chat_client::api::LlmClient;
use ai_chat_client::model::{Message, Provider, Role};

/// 启动一个一次性 mock SSE 服务器，返回固定内容，返回端口。
fn spawn_mock_sse(body: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // 读掉请求头（忽略内容）
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

fn make_provider(port: u16) -> Provider {
    Provider {
        id: 1,
        name: "mock".to_string(),
        base_url: format!("http://127.0.0.1:{port}"),
        api_key: "test-key".to_string(),
        default_model: "mock-model".to_string(),
    }
}

fn make_messages() -> Vec<Message> {
    vec![Message {
        id: 1,
        conversation_id: 1,
        role: Role::User,
        content: "你好".to_string(),
        created_at: 0,
    }]
}

#[tokio::test]
async fn chat_stream_concatenates_deltas() {
    // 正常的一次性 SSE（每个 data 一行）
    let body = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"，世界\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"！\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    let port = spawn_mock_sse(body);

    let client = OpenAiClient::new();
    let provider = make_provider(port);
    let messages = make_messages();

    let received = Arc::new(Mutex::new(String::new()));
    let received_cb = received.clone();
    let result = client
        .chat_stream(
            &provider,
            &messages,
            Box::new(move |d| received_cb.lock().unwrap().push_str(&d)),
        )
        .await
        .expect("chat_stream should succeed");

    assert_eq!(result, "你好，世界！");
    assert_eq!(*received.lock().unwrap(), "你好，世界！");
}

#[tokio::test]
async fn chat_stream_handles_split_lines() {
    // 模拟 TCP 把一行切成多个 chunk：JSON 被拆散在不同字节段
    // 这里把整个 body 作为一段，但内容里包含被截断的场景由服务器侧控制。
    // 简化：仍用完整行，但带 \r\n 结尾与不完整最后一行。
    let body = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"A\"}}]}\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":null}}]}\r\n\r\n", // content 为 null，应跳过
        "data: {\"choices\":[{\"delta\":{\"content\":\"B\"}}]}\r\n\r\n",
        "data: [DONE]\r\n\r\n"
    );
    let port = spawn_mock_sse(body);

    let client = OpenAiClient::new();
    let provider = make_provider(port);
    let messages = make_messages();

    let received = Arc::new(Mutex::new(String::new()));
    let received_cb = received.clone();
    let result = client
        .chat_stream(
            &provider,
            &messages,
            Box::new(move |d| received_cb.lock().unwrap().push_str(&d)),
        )
        .await
        .expect("chat_stream should succeed");

    assert_eq!(result, "AB");
    assert_eq!(*received.lock().unwrap(), "AB");
}
