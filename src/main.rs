// 作为 Windows GUI 应用构建，双击 exe 不再弹出控制台窗口。
#![windows_subsystem = "windows"]

mod app;

use ai_chat_client::config::AppConfig;
use gpui_kit::{px, size, AppContext, WindowBounds, WindowOptions};

use app::App;

fn main() {
    // GUI 模式没有控制台，崩溃时把 panic 信息 + 调用栈写入 panic.log 便于诊断。
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let msg = format!("PANIC: {info}\n\nBacktrace:\n{backtrace}\n");
        let _ = std::fs::write("panic.log", &msg);
        eprintln!("{msg}");
    }));

    // 首次运行在 exe 同目录生成 config.toml 模板，用户填入 API Key 即可。
    AppConfig::ensure_template();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            // 必须在使用任何 gpui-component 功能前调用
            gpui_kit::init(cx);

            // 窗口居中、大小适中，避免默认尺寸过大导致输入框被任务栏遮挡。
            let mut options = WindowOptions::default();
            options.window_bounds =
                Some(WindowBounds::centered(size(px(960.0), px(680.0)), cx));

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| App::new(window, cx))
            })
            .expect("failed to open window");
        });
}
