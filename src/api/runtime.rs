//! 进程级 Tokio runtime。
//!
//! reqwest 的异步 I/O（DNS、连接、流式读取）必须运行在真实的 Tokio reactor 上。
//! 而 GPUI 的 foreground / background executor 都不是 Tokio runtime，
//! 直接在其上 `await` reqwest 会 panic：`there is no reactor running`。
//!
//! 因此所有网络请求统一在该 runtime 上 `spawn`，再 await 其 JoinHandle
//! （JoinHandle 的轮询不依赖 reactor，可在任意 executor 上等待）。

use std::sync::OnceLock;

use tokio::runtime::Runtime;

/// 返回全局共享的 Tokio runtime（首次调用时创建）。
pub fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("failed to build tokio runtime"))
}
