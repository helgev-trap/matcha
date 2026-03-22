use dashmap::{DashMap, DashSet};
use fxhash::FxBuildHasher;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

use crate::window::{
    Window, WindowConfig, WindowControler, WindowError, WindowId, WindowRenderable,
};

pub struct WindowManager {
    windows: DashMap<WindowId, Arc<Mutex<Window>>, FxBuildHasher>,
    handles: DashMap<WindowId, Weak<WindowHandle>, FxBuildHasher>,
    disabled_windows: DashSet<WindowId, FxBuildHasher>,
    weak_self: Weak<Self>,
}

impl WindowManager {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            windows: DashMap::with_hasher(FxBuildHasher::default()),
            handles: DashMap::with_hasher(FxBuildHasher::default()),
            disabled_windows: DashSet::with_hasher(FxBuildHasher::default()),
            weak_self: weak_self.clone(),
        })
    }
}

impl WindowManager {
    pub fn create_window(
        &self,
        ctrl: &impl WindowControler,
        config: &WindowConfig,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<Arc<WindowHandle>, WindowError> {
        let window_surface = ctrl.create_native_window(config, instance, device)?;
        let id = window_surface.window_id();

        let window = Window::new(config.clone(), window_surface);

        self.windows.insert(id, Arc::new(Mutex::new(window)));
        self.disabled_windows.remove(&id);

        let handle = Arc::new(WindowHandle {
            id,
            weak_to_manager: self.weak_self.clone(),
        });
        self.handles.insert(id, Arc::downgrade(&handle));

        Ok(handle)
    }

    pub fn get_window_handle(&self, id: WindowId) -> Option<Arc<WindowHandle>> {
        self.handles.get(&id).and_then(|h| h.upgrade())
    }

    pub(crate) fn remove_window(&self, id: WindowId) {
        self.windows.remove(&id);
        self.disabled_windows.remove(&id);
        self.handles.remove(&id);
    }

    pub async fn disable_window(&self, id: WindowId) {
        if let Some(inner) = self.windows.get(&id) {
            let mut inner = inner.lock().await;
            inner.set_surface(None);
            self.disabled_windows.insert(id);
        }
    }

    pub async fn disable_all_windows(&self) {
        for iter in self.windows.iter() {
            let id = *iter.key();
            let mut inner = iter.lock().await;
            inner.set_surface(None);
            self.disabled_windows.insert(id);
        }
    }

    pub async fn enable_window(
        &self,
        id: WindowId,
        ctrl: &impl WindowControler,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<(), WindowError> {
        let inner_arc = if let Some(inner) = self.windows.get(&id) {
            inner.clone()
        } else {
            return Ok(());
        };

        let mut inner = inner_arc.lock().await;
        if inner.has_surface() {
            return Ok(()); // Already enabled
        }

        let window_surface = ctrl.create_native_window(inner.config(), instance, device)?;

        // We update the window_surface and keep the original WindowId to not break UiArch.
        inner.set_surface(Some(window_surface));
        self.disabled_windows.remove(&id);

        Ok(())
    }

    pub async fn enable_all_windows(
        &self,
        ctrl: &impl WindowControler,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<(), WindowError> {
        let disabled_ids: Vec<WindowId> = self.disabled_windows.iter().map(|id| *id).collect();
        for id in disabled_ids {
            self.enable_window(id, ctrl, instance, device).await?;
        }
        Ok(())
    }

    pub async fn with_window<R>(&self, id: WindowId, f: impl FnOnce(&mut Window) -> R) -> Option<R> {
        if let Some(inner) = self.windows.get(&id) {
            let mut inner = inner.lock().await;
            Some(f(&mut inner))
        } else {
            None
        }
    }
}

pub struct WindowHandle {
    id: WindowId,
    weak_to_manager: Weak<WindowManager>,
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        if let Some(manager) = self.weak_to_manager.upgrade() {
            manager.remove_window(self.id);
        }
    }
}

impl WindowHandle {
    pub async fn set_renderable(&self, renderable: impl Into<Arc<dyn WindowRenderable>>) {
        if let Some(manager) = self.weak_to_manager.upgrade() {
            manager
                .with_window(self.id, |window: &mut Window| {
                    window.set_renderable(Some(renderable.into()));
                })
                .await;
        }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }
}
