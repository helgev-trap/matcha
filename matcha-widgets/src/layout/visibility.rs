use matcha_core::context::WidgetContext;
use matcha_core::{
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{AnyWidget, AnyWidgetFrame, Background, Dom, InvalidationHandle, Widget, WidgetFrame},
};
use renderer::render_node::RenderNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityState {
    /// The widget is visible.
    Visible,
    /// The widget is hidden, but still takes up space.
    Hidden,
    /// The widget is completely removed from the layout.
    Gone,
}

pub struct Visibility<T>
where
    T: Send + 'static,
{
    label: Option<String>,
    visibility: VisibilityState,
    content: Option<Box<dyn Dom<T>>>,
}

impl<T> Visibility<T>
where
    T: Send + 'static,
{
    pub fn new() -> Self {
        Self {
            label: None,
            visibility: VisibilityState::Visible,
            content: None,
        }
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn visible(mut self) -> Self {
        self.visibility = VisibilityState::Visible;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visibility = VisibilityState::Hidden;
        self
    }

    pub fn gone(mut self) -> Self {
        self.visibility = VisibilityState::Gone;
        self
    }

    pub fn visibility(mut self, visibility: VisibilityState) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn content(mut self, content: impl Dom<T>) -> Self {
        self.content = Some(Box::new(content));
        self
    }
}

#[async_trait::async_trait]
impl<T> Dom<T> for Visibility<T>
where
    T: Send + 'static,
{
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        let mut children_and_settings = Vec::new();
        let mut child_ids = Vec::new();

        if let Some(content_widget) = self.content.as_ref().map(|c| c.build_widget_tree()) {
            children_and_settings.push((content_widget, ()));
            child_ids.push(0);
        }

        Box::new(WidgetFrame::new(
            self.label.clone(),
            children_and_settings,
            child_ids,
            VisibilityNode {
                visibility: self.visibility,
            },
        ))
    }
}

pub struct VisibilityNode {
    visibility: VisibilityState,
}

impl<T> Widget<Visibility<T>, T, ()> for VisibilityNode
where
    T: Send + 'static,
{
    fn update_widget<'a>(
        &mut self,
        dom: &'a Visibility<T>,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        if self.visibility != dom.visibility {
            // If visibility changed, we need to invalidate the cache
            if let Some(handle) = cache_invalidator {
                handle.relayout_next_frame();
            }
        }
        self.visibility = dom.visibility;

        dom.content
            .as_ref()
            .map(|c| (c.as_ref(), (), 0))
            .into_iter()
            .collect()
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        event: &DeviceInput,
        children: &mut [(&mut dyn AnyWidget<T>, &mut (), &Arrangement)],
        _cache_invalidator: InvalidationHandle,
        ctx: &WidgetContext,
    ) -> Option<T> {
        if self.visibility == VisibilityState::Visible {
            if let Some((child, _, _arrangement)) = children.first_mut() {
                return child.device_input(event, ctx);
            }
        }
        None
    }

    fn is_inside(
        &self,
        _bounds: [f32; 2],
        position: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        ctx: &WidgetContext,
    ) -> bool {
        match self.visibility {
            VisibilityState::Visible => {
                if let Some((child, _, _arrangement)) = children.first() {
                    return child.is_inside(position, ctx);
                }
                false
            }
            VisibilityState::Hidden | VisibilityState::Gone => false,
        }
    }

    fn measure(
        &self,
        constraints: &Constraints,
        children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        match self.visibility {
            VisibilityState::Visible | VisibilityState::Hidden => {
                if let Some((child, _)) = children.first() {
                    child.measure(constraints, ctx)
                } else {
                    [0.0, 0.0]
                }
            }
            VisibilityState::Gone => [0.0, 0.0],
        }
    }

    fn arrange(
        &self,
        bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        if let Some((child, _)) = children.first() {
            match self.visibility {
                VisibilityState::Visible | VisibilityState::Hidden => {
                    let measured_size = child.measure(&Constraints::from_max_size(bounds), ctx);
                    let final_size = [
                        measured_size[0].min(bounds[0]),
                        measured_size[1].min(bounds[1]),
                    ];

                    vec![Arrangement::new(final_size, nalgebra::Matrix4::identity())]
                }
                VisibilityState::Gone => vec![Arrangement::default()],
            }
        } else {
            vec![]
        }
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        if self.visibility == VisibilityState::Visible {
            if let Some((child, _, arrangement)) = children.first() {
                let affine = arrangement.affine;

                let child_node = child.render(background, ctx);

                return RenderNode::new().add_child(child_node, affine);
            }
        }
        RenderNode::default()
    }
}
