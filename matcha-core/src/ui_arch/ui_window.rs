use crate::{
    ui_arch::widget::{View, Widget, WidgetContext, WidgetPod},
    window::WindowConfig,
    window_manager::WindowHandle,
};

pub struct UiWindow<T: 'static> {
    // TODO: Add some field to config window.
    pub child_view: Box<dyn View<T>>,
}

impl<T: 'static> UiWindow<T> {
    pub fn new(child_view: impl Into<Box<dyn View<T>>>) -> Self {
        Self {
            child_view: child_view.into(),
        }
    }
}

impl<T: 'static> View<T> for UiWindow<T> {
    fn build(&self, ctx: &dyn WidgetContext) -> super::widget::WidgetPod<T> {
        let widget_pod = self.child_view.build(ctx);

        let ui_window_widget = UiWindowWidget {
            window_handle: ctx.create_window(&WindowConfig::default()),
            child_widget: widget_pod,
        };

        WidgetPod::new((), ui_window_widget)
    }
}

pub struct UiWindowWidget<T: 'static> {
    window_handle: WindowHandle,

    child_widget: WidgetPod<T>,
}

impl<T: 'static> Widget<T> for UiWindowWidget<T> {
    type View = UiWindow<T>;

    fn update(
        &mut self,
        view: &Self::View,
        ctx: &dyn WidgetContext,
    ) -> super::widget::WidgetInteractionResult {
        todo!()
    }

    fn device_input(
        &mut self,
        bounds: [f32; 2],
        event: &crate::event::device_event::DeviceEvent,
        ctx: &dyn WidgetContext,
    ) -> (Option<T>, super::widget::WidgetInteractionResult) {
        todo!()
    }

    fn measure(
        &self,
        constraints: &super::metrics::Constraints,
        ctx: &dyn WidgetContext,
    ) -> [f32; 2] {
        todo!()
    }

    fn render(&mut self, bounds: [f32; 2], ctx: &dyn WidgetContext) -> renderer::RenderNode {
        todo!()
    }
}
