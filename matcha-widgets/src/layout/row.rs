use matcha_core::context::WidgetContext;
use matcha_core::{
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{AnyWidget, AnyWidgetFrame, Background, Dom, InvalidationHandle, Widget, WidgetFrame},
};
use renderer::render_node::RenderNode;

use crate::types::grow_size::GrowSize;
use crate::types::size::ChildSize;
use crate::types::{
    flex::{AlignItems, JustifyContent},
    size::Size,
};

// MARK: DOM

pub struct Row<T>
where
    T: Send + 'static,
{
    label: Option<String>,
    justify_content: JustifyContent,
    align_items: AlignItems,
    items: Vec<Box<dyn Dom<T>>>,
}

impl<T> Row<T>
where
    T: Send + 'static,
{
    pub fn new(label: Option<&str>) -> Self {
        Self {
            label: label.map(String::from),
            justify_content: JustifyContent::FlexStart {
                gap: GrowSize::Fixed(Size::px(0.0)),
            },
            align_items: AlignItems::Start,
            items: Vec::new(),
        }
    }

    pub fn justify_content(mut self, justify_content: JustifyContent) -> Self {
        self.justify_content = justify_content;
        self
    }

    pub fn align_items(mut self, align_items: AlignItems) -> Self {
        self.align_items = align_items;
        self
    }

    pub fn push(mut self, item: impl Dom<T>) -> Self {
        self.items.push(Box::new(item));
        self
    }
}

#[async_trait::async_trait]
impl<T> Dom<T> for Row<T>
where
    T: Send + 'static,
{
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        let mut children_and_settings = Vec::new();
        let mut child_ids = Vec::new();

        for (index, item) in self.items.iter().enumerate() {
            let child_widget = item.build_widget_tree();
            children_and_settings.push((child_widget, ()));
            child_ids.push(index as u128);
        }

        Box::new(WidgetFrame::new(
            self.label.clone(),
            children_and_settings,
            child_ids,
            RowNode {
                justify_content: self.justify_content.clone(),
                align_items: self.align_items,
            },
        ))
    }
}

// MARK: Widget

pub struct RowNode {
    justify_content: JustifyContent,
    align_items: AlignItems,
}

impl RowNode {
    /// Calculate gap and initial offset for given justify_content.
    /// - container_size: full available width
    /// - total_child_width: sum of measured child widths
    /// - child_max_height: maximum child height (used when Size functions consult child size)
    /// - child_count: number of children
    fn calc_gap_and_offset(
        &self,
        justify_content: &JustifyContent,
        container_size: f32,
        total_child_width: f32,
        child_max_height: f32,
        child_count: usize,
        ctx: &WidgetContext,
    ) -> (f32, f32) {
        if child_count == 0 {
            return (0.0, 0.0);
        }

        let mut gap: f32;
        let mut offset: f32;

        // Provide representative ChildSize containing total child width and max height,
        // as requested: "gap 計算時、子要素の値として横幅の総和および最大縦幅を渡す"
        let mut rep_child_size = ChildSize::with_size([total_child_width, child_max_height]);

        match justify_content {
            JustifyContent::FlexStart { gap: g }
            | JustifyContent::FlexEnd { gap: g }
            | JustifyContent::Center { gap: g } => {
                // If gap is Grow, distribute available space evenly across gaps.
                match g {
                    GrowSize::Grow(s) => {
                        if child_count >= 2 {
                            let available_space = container_size - total_child_width;
                            let grow_val = s.size(
                                [container_size, child_max_height],
                                &mut rep_child_size,
                                ctx,
                            );
                            gap = (available_space / (child_count - 1) as f32 * grow_val).max(0.0);
                            offset = 0.0;
                        } else {
                            // single child: follow existing behaviour -> gap 0, offset depends on alignment
                            gap = 0.0;
                            offset = match justify_content {
                                JustifyContent::FlexEnd { .. } => {
                                    container_size - total_child_width
                                }
                                JustifyContent::Center { .. } => {
                                    (container_size - total_child_width) / 2.0
                                }
                                _ => 0.0,
                            };
                        }
                    }
                    // For Size::Size and other Size variants, evaluate the function.
                    GrowSize::Fixed(s) => {
                        gap = s.size([container_size, child_max_height], &mut rep_child_size, ctx);
                        offset = 0.0;
                    }
                }
            }
            JustifyContent::SpaceAround => {
                let available_space = container_size - total_child_width;
                gap = available_space / child_count as f32;
                offset = gap / 2.0;
            }
            JustifyContent::SpaceEvenly => {
                let available_space = container_size - total_child_width;
                gap = available_space / (child_count + 1) as f32;
                offset = gap;
            }
            JustifyContent::SpaceBetween => {
                let available_space = container_size - total_child_width;
                if child_count >= 2 {
                    gap = (available_space / (child_count - 1) as f32).max(0.0);
                    offset = 0.0;
                } else {
                    gap = 0.0;
                    offset = 0.0;
                }
            }
        }

        // Clamp gap to avoid negative spacing when children overflow
        gap = gap.max(0.0);

        // Recalculate offsets that depend on gap (FlexEnd / Center)
        offset = match justify_content {
            JustifyContent::FlexEnd { .. } => {
                container_size - total_child_width - gap * (child_count - 1) as f32
            }
            JustifyContent::Center { .. } => {
                (container_size - total_child_width - gap * (child_count - 1) as f32) / 2.0
            }
            JustifyContent::SpaceAround => gap / 2.0,
            JustifyContent::SpaceEvenly => gap,
            _ => offset,
        };

        (gap, offset)
    }
}

impl<T> Widget<Row<T>, T, ()> for RowNode
where
    T: Send + 'static,
{
    fn update_widget<'a>(
        &mut self,
        dom: &'a Row<T>,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        // Use the same conservative change-detection as Column: treat certain
        // JustifyContent variants as equivalent to avoid unnecessary relayouts.
        let justify_content_changed = !matches!(
            (&self.justify_content, &dom.justify_content),
            (
                JustifyContent::FlexStart { .. },
                JustifyContent::FlexStart { .. }
            ) | (
                JustifyContent::FlexEnd { .. },
                JustifyContent::FlexEnd { .. }
            ) | (JustifyContent::Center { .. }, JustifyContent::Center { .. })
                | (JustifyContent::SpaceBetween, JustifyContent::SpaceBetween)
                | (JustifyContent::SpaceAround, JustifyContent::SpaceAround)
                | (JustifyContent::SpaceEvenly, JustifyContent::SpaceEvenly)
        );

        if justify_content_changed || self.align_items != dom.align_items {
            if let Some(h) = cache_invalidator {
                h.relayout_next_frame()
            }
        }

        self.justify_content = dom.justify_content.clone();
        self.align_items = dom.align_items;

        dom.items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.as_ref(), (), index as u128))
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
        // Iterate children in reverse order so top-most (last) child receives events first.
        for (child, _, arrangement) in children.iter_mut().rev() {
            let child_event = event.transform(arrangement.affine);
            if let Some(result) = child.device_input(&child_event, ctx) {
                return Some(result);
            }
        }
        None
    }

    fn is_inside(
        &self,
        bounds: [f32; 2],
        position: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        _ctx: &WidgetContext,
    ) -> bool {
        0.0 <= position[0]
            && position[0] <= bounds[0]
            && 0.0 <= position[1]
            && position[1] <= bounds[1]
    }

    fn measure(
        &self,
        constraints: &Constraints,
        children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        if children.is_empty() {
            return [0.0, 0.0];
        }

        let mut total_child_width = 0.0f32;
        let mut max_child_height = 0.0f32;

        // Measure all children using final constraints (same approach as Column)
        for (child, _) in children {
            let child_size = child.measure(constraints, ctx);
            total_child_width += child_size[0];
            max_child_height = max_child_height.max(child_size[1]);
        }

        // Compute gap using helper (accounts for Grow and space distribution)
        let (gap, _offset) = self.calc_gap_and_offset(
            &self.justify_content,
            constraints.max_width(),
            total_child_width,
            max_child_height,
            children.len(),
            ctx,
        );

        if children.len() > 1 {
            total_child_width += gap * (children.len() - 1) as f32;
        }

        [
            total_child_width.min(constraints.max_width()),
            max_child_height.min(constraints.max_height()),
        ]
    }

    fn arrange(
        &self,
        bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        if children.is_empty() {
            return vec![];
        }

        // Measure children to get their preferred sizes
        let child_constraints = Constraints::new([0.0, bounds[0]], [0.0, bounds[1]]);
        let child_sizes: Vec<[f32; 2]> = children
            .iter()
            .map(|(child, _)| child.measure(&child_constraints, ctx))
            .collect();

        let mut total_child_width: f32 = 0.0;
        let mut child_max_height: f32 = 0.0;

        for child_size in &child_sizes {
            total_child_width += child_size[0];
            child_max_height = child_max_height.max(child_size[1]);
        }

        let child_count = child_sizes.len();

        // Use helper to compute gap and offset. Per user's instruction, pass total width and max height.
        let (gap, offset) = self.calc_gap_and_offset(
            &self.justify_content,
            bounds[0],
            total_child_width,
            child_max_height,
            child_count,
            ctx,
        );

        let mut accumulate_width = offset;
        let mut arrangements = Vec::with_capacity(children.len());

        for child_size in &child_sizes {
            let child_width = child_size[0];
            let child_height = child_size[1];

            // Vertical alignment
            let y = match self.align_items {
                AlignItems::Start => 0.0,
                AlignItems::End => (bounds[1] - child_height).max(0.0),
                AlignItems::Center => ((bounds[1] - child_height) / 2.0).max(0.0),
            };

            let arrangement = Arrangement::new(
                *child_size,
                nalgebra::Matrix4::new_translation(&nalgebra::Vector3::new(
                    accumulate_width,
                    y,
                    0.0,
                )),
            );
            arrangements.push(arrangement);

            accumulate_width += child_width + gap;
        }

        arrangements
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        let mut render_node = RenderNode::new();

        for (child, _, arrangement) in children {
            let affine = arrangement.affine;

            let child_node = child.render(background, ctx);
            render_node = render_node.add_child(child_node, affine);
        }

        render_node
    }
}
