use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use gpui_kit::component::{
    ActiveTheme, StyledExt as _, WindowExt,
    button::{Button, ButtonVariants},
    input::{Textarea, TextareaState},
    scroll::ScrollableElement,
    text::TextView,
};
use gpui_kit::base::Disableable;
use gpui_kit::{
    AppContext as _, ClipboardItem, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, Window, div, px, relative,
};

use ai_chat_client::api::openai_compatible::OpenAiClient;
use ai_chat_client::db::{conversation_repo, message_repo, provider_repo, Database};
use ai_chat_client::model::{Conversation, Message, Role};
use ai_chat_client::service::ChatService;

/// 应用根视图。
pub struct App {
    db: Arc<Database>,
    chat_service: Arc<ChatService>,
    provider_id: i64,
    conversations: Vec<Conversation>,
    messages: Vec<Message>,
    input: Entity<TextareaState>,
    active_conversation_id: i64,
    is_loading: bool,
    error: Option<String>,
    copied_idx: Option<usize>,
    /// 待确认删除的会话 id（非 None 时显示确认弹层）。
    pending_delete: Option<i64>,
    _subscriptions: Vec<Subscription>,
}

impl App {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 打开本地数据库
        let db_path = "ai_chat.db";
        let db = Arc::new(Database::open(db_path).expect("failed to open database"));

        // Phase 1：确保有一个默认 Provider 与会话
        let provider_id = db
            .with_conn(|conn| {
                if let Some(p) = provider_repo::get_first(conn)? {
                    Ok(p.id)
                } else {
                    // 默认指向 DeepSeek（OpenAI 兼容协议）；Key 通过环境变量配置
                    provider_repo::insert(
                        conn,
                        "DeepSeek",
                        "https://api.deepseek.com",
                        "",
                        "deepseek-chat",
                    )
                }
            })
            .expect("failed to init provider");

        let now = chrono::Utc::now().timestamp();
        let conversation_id = db
            .with_conn(|conn| {
                let convs = conversation_repo::list_all(conn)?;
                if let Some(c) = convs.into_iter().next() {
                    Ok(c.id)
                } else {
                    conversation_repo::insert(conn, "新对话", provider_id, now)
                }
            })
            .expect("failed to init conversation");

        // 加载历史消息
        let messages = db
            .with_conn(|conn| message_repo::list_by_conversation(conn, conversation_id))
            .unwrap_or_default();

        // 加载会话列表（侧边栏）
        let conversations = db
            .with_conn(|conn| conversation_repo::list_all(conn))
            .unwrap_or_default();

        // 初始化 LLM 客户端与服务
        let llm: Arc<dyn ai_chat_client::api::LlmClient> = Arc::new(OpenAiClient::new());
        let chat_service = Arc::new(ChatService::new(db.clone(), llm));

        // 输入框状态
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .placeholder("输入消息，按 Enter 发送（Shift+Enter 换行）")
        });

        Self {
            db,
            chat_service,
            provider_id,
            conversations,
            messages,
            input,
            active_conversation_id: conversation_id,
            is_loading: false,
            error: None,
            copied_idx: None,
            pending_delete: None,
            _subscriptions: Vec::new(),
        }
    }

    /// 发送当前输入框中的消息。
    fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.input.read(cx).value().to_string();
        if content.trim().is_empty() || self.is_loading {
            return;
        }

        // 清空输入框
        self.input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });

        let now = chrono::Utc::now().timestamp();
        let conv_id = self.active_conversation_id;

        // 先在 UI 追加用户消息
        self.messages.push(Message {
            id: -1,
            conversation_id: conv_id,
            role: Role::User,
            content: content.clone(),
            created_at: now,
        });
        // 追加一个空的 assistant 占位消息，流式增量会实时填充它
        self.messages.push(Message {
            id: -1,
            conversation_id: conv_id,
            role: Role::Assistant,
            content: String::new(),
            created_at: now,
        });
        let assistant_idx = self.messages.len() - 1;
        self.is_loading = true;
        self.error = None;
        cx.notify();

        let service = self.chat_service.clone();
        cx.spawn(async move |this, cx| {
            // channel：后台 HTTP 任务把每个 delta 发回前台用于实时渲染
            let (tx, mut rx) = futures::channel::mpsc::unbounded::<String>();

            // 后台任务：发起流式请求（reqwest 需 Tokio，故放到 background executor）
            let service = service.clone();
            let bg = cx.background_executor().spawn(async move {
                service
                    .send_message_stream(conv_id, &content, move |delta| {
                        let _ = tx.unbounded_send(delta);
                    })
                    .await
            });

            // 前台：持续接收 delta，追加到占位消息并触发重渲染（打字机效果）
            while let Some(delta) = rx.next().await {
                let _ = this.update(cx, |this, cx| {
                    if let Some(m) = this.messages.get_mut(assistant_idx) {
                        m.content.push_str(&delta);
                    }
                    cx.notify();
                });
            }

            // 收尾：读取后台任务结果，处理错误 / 复位 loading，并刷新侧边栏（标题/排序）
            match bg.await {
                Ok(_) => {
                    let _ = this.update(cx, |this, cx| {
                        this.is_loading = false;
                        this.reload_conversations(cx);
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.is_loading = false;
                        this.error = Some(e.to_string());
                        this.reload_conversations(cx);
                    });
                }
            }
        })
        .detach();
    }

    /// 复制某条消息内容到剪贴板，并短暂显示"已复制"。
    fn copy_message(&mut self, idx: usize, content: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(content));
        self.copied_idx = Some(idx);
        cx.notify();

        // 1.5 秒后清除"已复制"提示
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.copied_idx == Some(idx) {
                    this.copied_idx = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 重新加载侧边栏会话列表。
    fn reload_conversations(&mut self, cx: &mut Context<Self>) {
        self.conversations = self
            .db
            .with_conn(|conn| conversation_repo::list_all(conn))
            .unwrap_or_default();
        cx.notify();
    }

    /// 新建一个空会话并切换过去。标题带时间戳，避免多个"新对话"无法区分。
    fn new_conversation(&mut self, cx: &mut Context<Self>) {
        let now = chrono::Utc::now().timestamp();
        let title = format!("新对话 {}", chrono::Local::now().format("%m-%d %H:%M"));
        let id = self
            .db
            .with_conn(|conn| conversation_repo::insert(conn, &title, self.provider_id, now))
            .expect("failed to create conversation");
        self.reload_conversations(cx);
        self.switch_conversation(id, cx);
    }

    /// 切换到指定会话并加载其消息。
    fn switch_conversation(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }
        self.active_conversation_id = id;
        self.messages = self
            .db
            .with_conn(|conn| message_repo::list_by_conversation(conn, id))
            .unwrap_or_default();
        self.error = None;
        self.copied_idx = None;
        cx.notify();
    }

    /// 请求删除会话：先显示确认弹层，确认后才真正删除。
    fn request_delete(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }
        self.pending_delete = Some(id);
        cx.notify();
    }

    /// 删除会话；若删除的是当前会话，则切换到最新一个（没有则新建）。
    fn delete_conversation(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }
        let _ = self.db.with_conn(|conn| conversation_repo::delete(conn, id));
        self.reload_conversations(cx);
        if self.active_conversation_id == id {
            match self.conversations.first() {
                Some(c) => {
                    let next_id = c.id;
                    self.switch_conversation(next_id, cx);
                }
                None => self.new_conversation(cx),
            }
        }
    }

    /// 渲染左侧会话侧边栏。
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(200.))
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .p_2()
            .gap_2()
            .child(
                Button::new("new-conv")
                    .label("+ 新建对话")
                    .primary()
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.new_conversation(cx);
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .gap_1()
                    .children(self.conversations.iter().map(|c| {
                        let id = c.id;
                        let active = id == self.active_conversation_id;
                        let mut title: String = c.title.chars().take(12).collect();
                        if c.title.chars().count() > 12 {
                            title.push('…');
                        }
                        let btn = Button::new(("conv", id as usize)).label(title);
                        let btn = if active { btn.primary() } else { btn.ghost() };
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div().flex_1().overflow_hidden().child(
                                    btn.on_click(cx.listener(move |this, _, _window, cx| {
                                        this.switch_conversation(id, cx);
                                    })),
                                ),
                            )
                            .child(
                                Button::new(("del-conv", id as usize))
                                    .ghost()
                                    .label("×")
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.request_delete(id, cx);
                                    })),
                            )
                    })),
            )
    }
}

impl Render for App {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.messages.clone();
        let is_loading = self.is_loading;
        let error = self.error.clone();
        // 仅在等待首个 token 时显示"思考中"；开始流式输出后由占位消息本身呈现
        let waiting_first_token =
            is_loading && messages.last().map(|m| m.content.is_empty()).unwrap_or(true);

        let sidebar = self.render_sidebar(cx);
        // 待确认删除的会话（id + 标题），用于渲染确认弹层
        let pending_delete = self.pending_delete.and_then(|id| {
            self
                .conversations
                .iter()
                .find(|c| c.id == id)
                .map(|c| (id, c.title.clone()))
        });

        div()
            .flex()
            .flex_row()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .child(
                        // 消息列表（可滚动）
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .gap_3()
                    .p_4()
                    .children(messages.iter().enumerate().map(|(idx, m)| {
                        let is_user = matches!(m.role, Role::User);
                        let copied = self.copied_idx == Some(idx);
                        let content = m.content.clone();
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                // 头部：角色标签 + 复制按钮
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(if is_user { "你" } else { "AI" }),
                                    )
                                    .child(
                                        Button::new(("copy", idx))
                                            .link()
                                            .label(if copied { "已复制" } else { "复制" })
                                            .on_click(cx.listener(move |this, _, _window, cx| {
                                                this.copy_message(idx, content.clone(), cx);
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    // 气泡宽度随窗口缩放（最大占消息区 85%，最小 240px）
                                    .min_w(px(240.))
                                    .max_w(relative(0.85))
                                    .p_3()
                                    .rounded_lg()
                                    .bg(if is_user {
                                        cx.theme().accent
                                    } else {
                                        cx.theme().secondary
                                    })
                                    .child(if is_user {
                                        // 用户消息按纯文本展示
                                        div().child(m.content.clone()).into_any_element()
                                    } else {
                                        // AI 消息渲染 Markdown（标题/列表/表格/代码块语法高亮），文本可选择
                                        TextView::markdown(("md", idx), m.content.clone())
                                            .selectable(true)
                                            .into_any_element()
                                    }),
                            )
                    }))
                    .child(if waiting_first_token {
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("AI 正在思考…")
                    } else {
                        div().child("")
                    })
                    .children(error.map(|e| {
                        div()
                            .text_color(cx.theme().danger)
                            .child(format!("错误: {e}"))
                    })),
            )
            .child(
                // 底部输入区
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .p_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().child(Textarea::new(&self.input)))
                    .child(
                        Button::new("send")
                            .label("发送")
                            .primary()
                            .disabled(is_loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.send_message(window, cx);
                            })),
                    ),
            ),
            )
            // 删除确认弹层：覆盖整个窗口，仅当有待删除会话时渲染
            .children(pending_delete.map(|(id, title)| {
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui_kit::rgba(0x00000099))
                    .child(
                        div()
                            .w(px(360.))
                            .p_4()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(div().text_size(px(15.)).child("删除会话"))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "确定删除「{}」吗？该会话的所有消息将一并删除，且无法恢复。",
                                        title
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("cancel-delete")
                                            .label("取消")
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                this.pending_delete = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("confirm-delete")
                                            .label("删除")
                                            .danger()
                                            .on_click(cx.listener(move |this, _, _window, cx| {
                                                this.pending_delete = None;
                                                this.delete_conversation(id, cx);
                                            })),
                                    ),
                            ),
                    )
            }))
    }
}
