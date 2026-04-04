use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct UpdateFlags {
    flag: AtomicBool,
}

impl UpdateFlags {
    pub const fn new_true() -> Self {
        Self { flag: AtomicBool::new(true) }
    }

    pub const fn new_false() -> Self {
        Self { flag: AtomicBool::new(false) }
    }

    pub fn set(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    pub fn value(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }

    /// バックグラウンドタスクに渡すための軽量ハンドルを返す。
    /// `self` は `'static` なストレージ（`static` 変数）に格納されている必要がある。
    pub fn wakeup_handle(&'static self) -> WakeupHandle {
        WakeupHandle { flag: &self.flag }
    }
}

/// バックグラウンドスレッドからイベントループを起こすための軽量ハンドル。
/// `Copy + Clone + Send + 'static` なので非同期タスクへの `move` キャプチャが可能。
///
/// [`UpdateFlags::wakeup_handle`] で取得する。
#[derive(Clone, Copy)]
pub(crate) struct WakeupHandle {
    flag: &'static AtomicBool,
}

impl WakeupHandle {
    /// モデル更新が保留中であることをシグナルする。任意のスレッドから呼び出し可能。
    pub fn wake(&self) {
        self.flag.store(true, Ordering::Relaxed);
        // TODO: winit の EventLoopProxy::send_event() をここで呼ぶことで
        // 次のポーリングを待たずにイベントループを即時起床させることができる。
    }
}
