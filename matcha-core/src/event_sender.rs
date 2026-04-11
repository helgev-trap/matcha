use std::any::Any;

/// ウィジェットツリーからアプリケーション層へイベントを送るハンドル。
///
/// `'static + Clone + Send + Sync`。
///
/// `ctx.event_sender()` でクローンを取得し、`ctx.runtime_handle().spawn(async move { ... })` 内に
/// `move` して使う。
///
/// # 例
///
/// ```rust
/// fn setup(&self, ctx: &dyn UiContext) {
///     let sender = ctx.event_sender();
///     ctx.runtime_handle().spawn(async move {
///         // 非同期処理...
///         sender.emit(Box::new(MyEvent::Done));
///     });
/// }
/// ```
#[derive(Clone)]
pub struct EventSender {
    tx: tokio::sync::mpsc::UnboundedSender<Box<dyn Any + Send>>,
}

impl EventSender {
    pub(crate) fn channel() -> (EventSender, EventReceiver) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (EventSender { tx }, EventReceiver { rx })
    }

    /// 型消去されたイベントを送信する。同期・ノンブロッキング。
    ///
    /// レシーバ（`EventReceiver`）が既にドロップされている場合はサイレントに無視する。
    pub fn emit(&self, event: Box<dyn Any + Send>) {
        let _ = self.tx.send(event);
    }
}

/// ウィジェットツリーから送られたイベントを受け取るハンドル。
///
/// `Application::new()` の戻り値として返される。
/// `downcast_ref::<T>()` で具体型に復元する。
pub struct EventReceiver {
    rx: tokio::sync::mpsc::UnboundedReceiver<Box<dyn Any + Send>>,
}

impl EventReceiver {
    /// 次のイベントを非同期に受け取る。チャンネルが閉じると `None` を返す。
    pub async fn recv(&mut self) -> Option<Box<dyn Any + Send>> {
        self.rx.recv().await
    }

    /// ブロックせずにイベントを取り出す。
    pub fn try_recv(
        &mut self,
    ) -> Result<Box<dyn Any + Send>, tokio::sync::mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }
}
