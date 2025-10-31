use matcha_core::context::WidgetContext;
use matcha_core::metrics::{Arrangement, Constraints};
use nalgebra::Matrix4;

use matcha_core::ui::widget::InvalidationHandle;
use matcha_core::{
    device_input::DeviceInput,
    ui::{AnyWidget, AnyWidgetFrame, Background, Dom, Widget, WidgetFrame},
};
use renderer::render_node::RenderNode;

use crate::types::flex::{AlignItems, JustifyContent};
use crate::types::grow_size::GrowSize;
use crate::types::size::{ChildSize, Size};

// MARK: DOM

pub struct Column<T>
where
    T: Send + 'static,
{
    label: Option<String>,
    justify_content: JustifyContent,
    align_items: AlignItems,
    items: Vec<Box<dyn Dom<T>>>,
}

impl<T> Column<T>
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
impl<T> Dom<T> for Column<T>
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
            ColumnNode {
                justify_content: self.justify_content.clone(),
                align_items: self.align_items,
            },
        ))
    }
}

// MARK: Widget

pub struct ColumnNode {
    justify_content: JustifyContent,
    align_items: AlignItems,
}

impl ColumnNode {
    /// Calculate gap and initial offset for given justify_content (vertical axis).
    /// - container_size: full available height
    /// - total_child_height: sum of measured child heights
    /// - child_max_width: maximum child width (used when Size functions consult child size)
    /// - child_count: number of children
    fn calc_gap_and_offset(
        &self,
        justify_content: &JustifyContent,
        container_size: f32,
        total_child_height: f32,
        child_max_width: f32,
        child_count: usize,
        ctx: &WidgetContext,
    ) -> (f32, f32) {
        if child_count == 0 {
            return (0.0, 0.0);
        }

        let mut gap: f32;
        let mut offset: f32;

        // Representative ChildSize: [width, height]
        let mut rep_child_size = ChildSize::with_size([child_max_width, total_child_height]);

        match justify_content {
            JustifyContent::FlexStart { gap: g }
            | JustifyContent::FlexEnd { gap: g }
            | JustifyContent::Center { gap: g } => {
                match g {
                    GrowSize::Grow(s) => {
                        if child_count >= 2 {
                            let available_space = container_size - total_child_height;
                            let grow_val =
                                s.size([child_max_width, container_size], &mut rep_child_size, ctx);
                            gap = (available_space / (child_count - 1) as f32 * grow_val).max(0.0);
                            offset = 0.0;
                        } else {
                            // single child: gap 0, offset depends on alignment
                            gap = 0.0;
                            offset = match justify_content {
                                JustifyContent::FlexEnd { .. } => {
                                    container_size - total_child_height
                                }
                                JustifyContent::Center { .. } => {
                                    (container_size - total_child_height) / 2.0
                                }
                                _ => 0.0,
                            };
                        }
                    }
                    GrowSize::Fixed(s) => {
                        gap = s.size([child_max_width, container_size], &mut rep_child_size, ctx);
                        offset = 0.0;
                    }
                }
            }
            JustifyContent::SpaceAround => {
                let available_space = container_size - total_child_height;
                gap = available_space / child_count as f32;
                offset = gap / 2.0;
            }
            JustifyContent::SpaceEvenly => {
                let available_space = container_size - total_child_height;
                gap = available_space / (child_count + 1) as f32;
                offset = gap;
            }
            JustifyContent::SpaceBetween => {
                let available_space = container_size - total_child_height;
                if child_count >= 2 {
                    gap = (available_space / (child_count - 1) as f32).max(0.0);
                    offset = 0.0;
                } else {
                    gap = 0.0;
                    offset = 0.0;
                }
            }
        }

        gap = gap.max(0.0);

        offset = match justify_content {
            JustifyContent::FlexEnd { .. } => {
                container_size - total_child_height - gap * (child_count - 1) as f32
            }
            JustifyContent::Center { .. } => {
                (container_size - total_child_height - gap * (child_count - 1) as f32) / 2.0
            }
            JustifyContent::SpaceAround => gap / 2.0,
            JustifyContent::SpaceEvenly => gap,
            _ => offset,
        };

        (gap, offset)
    }
}

impl<T> Widget<Column<T>, T, ()> for ColumnNode
where
    T: Send + 'static,
{
    fn update_widget<'a>(
        &mut self,
        dom: &'a Column<T>,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        // Check if justify_content or align_items changed
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
        // Process children in reverse order for proper event handling
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

        let mut total_height = 0.0f32;
        let mut max_width = 0.0f32;

        // Measure all children
        for (child, _) in children {
            let child_size = child.measure(constraints, ctx);
            total_height += child_size[1];
            max_width = max_width.max(child_size[0]);
        }

        // Compute gap using helper (accounts for Grow and space distribution)
        let (gap, _offset) = self.calc_gap_and_offset(
            &self.justify_content,
            constraints.max_height(),
            total_height,
            max_width,
            children.len(),
            ctx,
        );

        if children.len() > 1 {
            total_height += gap * (children.len() - 1) as f32;
        }

        [
            max_width.min(constraints.max_width()),
            total_height.min(constraints.max_height()),
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

        // Measure children to get their preferred sizes constrained by final size
        let child_constraints = Constraints::new([0.0, bounds[0]], [0.0, bounds[1]]);
        let child_sizes: Vec<[f32; 2]> = children
            .iter()
            .map(|(child, _)| child.measure(&child_constraints, ctx))
            .collect();

        let total_child_height: f32 = child_sizes.iter().map(|s| s[1]).sum();

        // Compute child max width for Size evaluation and get gap + starting offset
        let mut child_max_width: f32 = 0.0;
        for s in &child_sizes {
            child_max_width = child_max_width.max(s[0]);
        }

        let (gap, mut y_offset) = self.calc_gap_and_offset(
            &self.justify_content,
            bounds[1],
            total_child_height,
            child_max_width,
            child_sizes.len(),
            ctx,
        );

        let mut arrangements = Vec::new();

        for (index, &child_size) in child_sizes.iter().enumerate() {
            // Calculate x offset based on align_items (cross-axis)
            let x_offset = match self.align_items {
                AlignItems::Start => 0.0,
                AlignItems::End => bounds[0] - child_size[0],
                AlignItems::Center => (bounds[0] - child_size[0]) / 2.0,
            };

            let transform =
                Matrix4::new_translation(&nalgebra::Vector3::new(x_offset, y_offset, 0.0));
            arrangements.push(Arrangement::new(child_size, transform));

            // Calculate spacing for next child (vertical spacing)
            let spacing = match &self.justify_content {
                JustifyContent::FlexStart { .. }
                | JustifyContent::FlexEnd { .. }
                | JustifyContent::Center { .. } => gap,
                JustifyContent::SpaceBetween => {
                    if children.len() > 1 && index < children.len() - 1 {
                        (bounds[1] - total_child_height) / (children.len() - 1) as f32
                    } else {
                        0.0
                    }
                }
                JustifyContent::SpaceAround => {
                    if children.len() > 1 {
                        (bounds[1] - total_child_height) / children.len() as f32
                    } else {
                        0.0
                    }
                }
                JustifyContent::SpaceEvenly => {
                    if children.len() > 1 {
                        (bounds[1] - total_child_height) / (children.len() + 1) as f32
                    } else {
                        0.0
                    }
                }
            };

            y_offset += child_size[1] + spacing;
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
