use nalgebra::Matrix4;

use matcha_core::context::WidgetContext;
use matcha_core::ui::widget::InvalidationHandle;
use matcha_core::{
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{AnyWidget, AnyWidgetFrame, Background, Dom, Widget, WidgetFrame},
};
use renderer::render_node::RenderNode;

use crate::types::{
    grow_size::GrowSize,
    size::{ChildSize, Size},
};

// MARK: Dom

pub struct Grid<T: Send + 'static> {
    label: Option<String>,
    template_columns: Vec<GrowSize>,
    template_rows: Vec<GrowSize>,
    gap_columns: Size,
    gap_rows: Size,
    items: Vec<GridItem<T>>,
}

pub struct GridItem<T: Send + 'static> {
    pub item: Box<dyn Dom<T>>,
    pub column: [usize; 2],
    pub row: [usize; 2],
}

impl<T: Send + 'static> Grid<T> {
    pub fn new() -> Self {
        Self {
            label: None,
            template_columns: Vec::new(),
            template_rows: Vec::new(),
            gap_columns: Size::px(0.0),
            gap_rows: Size::px(0.0),
            items: Vec::new(),
        }
    }

    pub fn label(mut self, label: Option<String>) -> Self {
        self.label = label;
        self
    }

    pub fn template_columns(mut self, columns: Vec<GrowSize>) -> Self {
        self.template_columns = columns;
        self
    }

    pub fn template_rows(mut self, rows: Vec<GrowSize>) -> Self {
        self.template_rows = rows;
        self
    }

    pub fn gap_columns(mut self, gap: Size) -> Self {
        self.gap_columns = gap;
        self
    }

    pub fn gap_rows(mut self, gap: Size) -> Self {
        self.gap_rows = gap;
        self
    }

    pub fn item(
        mut self,
        item: impl Dom<T> + 'static,
        column: [usize; 2],
        row: [usize; 2],
    ) -> Self {
        self.items.push(GridItem {
            item: Box::new(item),
            column,
            row,
        });
        self
    }
}

#[async_trait::async_trait]
impl<T: Send + 'static> Dom<T> for Grid<T> {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        let mut children_and_settings = Vec::new();
        let mut child_ids = Vec::new();

        for (index, grid_item) in self.items.iter().enumerate() {
            let child_widget = grid_item.item.build_widget_tree();
            let grid_child_setting = GridChildSetting {
                column: grid_item.column,
                row: grid_item.row,
            };
            children_and_settings.push((child_widget, grid_child_setting));
            child_ids.push(index as u128);
        }

        Box::new(WidgetFrame::new(
            self.label.clone(),
            children_and_settings,
            child_ids,
            GridNode {
                template_columns: self.template_columns.clone(),
                template_rows: self.template_rows.clone(),
                gap_columns: self.gap_columns.clone(),
                gap_rows: self.gap_rows.clone(),
                column_ranges: Vec::new(),
                row_ranges: Vec::new(),
            },
        ))
    }
}

// MARK: Widget

#[derive(Clone, PartialEq)]
pub struct GridChildSetting {
    pub column: [usize; 2],
    pub row: [usize; 2],
}

pub struct GridNode {
    template_columns: Vec<GrowSize>,
    template_rows: Vec<GrowSize>,
    gap_columns: Size,
    gap_rows: Size,
    column_ranges: Vec<[f32; 2]>,
    row_ranges: Vec<[f32; 2]>,
}

impl<T> Widget<Grid<T>, T, GridChildSetting> for GridNode
where
    T: Send + 'static,
{
    fn update_widget<'a>(
        &mut self,
        dom: &'a Grid<T>,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, GridChildSetting, u128)> {
        // Check if template or gap settings changed
        if self.template_columns != dom.template_columns
            || self.template_rows != dom.template_rows
            || self.gap_columns != dom.gap_columns
            || self.gap_rows != dom.gap_rows
        {
            if let Some(handle) = cache_invalidator {
                handle.relayout_next_frame();
            }
        }

        self.template_columns = dom.template_columns.clone();
        self.template_rows = dom.template_rows.clone();
        self.gap_columns = dom.gap_columns.clone();
        self.gap_rows = dom.gap_rows.clone();

        dom.items
            .iter()
            .enumerate()
            .map(|(index, grid_item)| {
                (
                    grid_item.item.as_ref(),
                    GridChildSetting {
                        column: grid_item.column,
                        row: grid_item.row,
                    },
                    index as u128,
                )
            })
            .collect()
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        event: &DeviceInput,
        children: &mut [(&mut dyn AnyWidget<T>, &mut GridChildSetting, &Arrangement)],
        _cache_invalidator: InvalidationHandle,
        ctx: &WidgetContext,
    ) -> Option<T> {
        for (child, _, arrangement) in children.iter_mut() {
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
        children: &[(&dyn AnyWidget<T>, &GridChildSetting, &Arrangement)],
        ctx: &WidgetContext,
    ) -> bool {
        // Check if position is within grid bounds
        if position[0] < 0.0
            || position[0] > bounds[0]
            || position[1] < 0.0
            || position[1] > bounds[1]
        {
            return false;
        }

        // Check if position is within any child
        for (child, _, arrangement) in children {
            let local_position = arrangement.to_local(position);
            if child.is_inside(local_position, ctx) {
                return true;
            }
        }

        false
    }

    fn measure(
        &self,
        constraints: &Constraints,
        children: &[(&dyn AnyWidget<T>, &GridChildSetting)],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        if self.template_columns.is_empty() || self.template_rows.is_empty() {
            return [0.0, 0.0];
        }

        let parent_size = [constraints.max_width(), constraints.max_height()];
        let (column_ranges, row_ranges) = self.calc_grid_layout(parent_size, ctx);

        let total_width = column_ranges.last().map(|r| r[1]).unwrap_or(0.0);
        let total_height = row_ranges.last().map(|r| r[1]).unwrap_or(0.0);

        [
            total_width
                .min(constraints.max_width())
                .max(constraints.min_width()),
            total_height
                .min(constraints.max_height())
                .max(constraints.min_height()),
        ]
    }

    fn arrange(
        &self,
        bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &GridChildSetting)],
        ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        let parent_size = [bounds[0], bounds[1]];
        let (column_ranges, row_ranges) = self.calc_grid_layout(parent_size, ctx);

        children
            .iter()
            .map(|(_, setting)| {
                let col_start = column_ranges
                    .get(setting.column[0])
                    .map(|r| r[0])
                    .unwrap_or(0.0);
                let col_end = column_ranges
                    .get(setting.column[1].saturating_sub(1))
                    .map(|r| r[1])
                    .unwrap_or(col_start);
                let row_start = row_ranges.get(setting.row[0]).map(|r| r[0]).unwrap_or(0.0);
                let row_end = row_ranges
                    .get(setting.row[1].saturating_sub(1))
                    .map(|r| r[1])
                    .unwrap_or(row_start);

                let child_size = [
                    (col_end - col_start).max(0.0),
                    (row_end - row_start).max(0.0),
                ];

                let transform =
                    Matrix4::new_translation(&nalgebra::Vector3::new(col_start, row_start, 0.0));

                Arrangement::new(child_size, transform)
            })
            .collect()
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &GridChildSetting, &Arrangement)],
        background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        let mut render_node = RenderNode::new();

        for (child, _, arrangement) in children {
            let child_node = child.render(background, ctx);
            render_node = render_node.add_child(child_node, arrangement.affine);
        }

        render_node
    }
}

impl GridNode {
    fn calc_grid_layout(
        &self,
        parent_size: [f32; 2],
        context: &WidgetContext,
    ) -> (Vec<[f32; 2]>, Vec<[f32; 2]>) {
        let (column_px_sum, column_grow_sum) =
            self.template_columns
                .iter()
                .fold((0.0, 0.0), |(sum, grow_sum), size| match size {
                    GrowSize::Fixed(s) => (
                        sum + s.size(parent_size, &mut ChildSize::default(), context),
                        grow_sum,
                    ),
                    GrowSize::Grow(s) => (
                        sum,
                        grow_sum + s.size(parent_size, &mut ChildSize::default(), context),
                    ),
                });

        let (row_px_sum, row_grow_sum) =
            self.template_rows
                .iter()
                .fold((0.0, 0.0), |(sum, grow_sum), size| match size {
                    GrowSize::Fixed(s) => (
                        sum + s.size(parent_size, &mut ChildSize::default(), context),
                        grow_sum,
                    ),
                    GrowSize::Grow(s) => (
                        sum,
                        grow_sum + s.size(parent_size, &mut ChildSize::default(), context),
                    ),
                });

        let column_gap_px = self
            .gap_columns
            .size(parent_size, &mut ChildSize::default(), context);
        let total_column_gap =
            column_gap_px * (self.template_columns.len().saturating_sub(1) as f32);

        let row_gap_px = self
            .gap_rows
            .size(parent_size, &mut ChildSize::default(), context);
        let total_row_gap = row_gap_px * (self.template_rows.len().saturating_sub(1) as f32);

        let column_px_per_grow = if column_grow_sum > 0.0 {
            ((parent_size[0] - column_px_sum - total_column_gap) / column_grow_sum).max(0.0)
        } else {
            0.0
        };

        let row_px_per_grow = if row_grow_sum > 0.0 {
            ((parent_size[1] - row_px_sum - total_row_gap) / row_grow_sum).max(0.0)
        } else {
            0.0
        };

        let mut column_ranges = Vec::with_capacity(self.template_columns.len());
        let mut current_x = 0.0;
        for size in &self.template_columns {
            let start = current_x;
            let width = match size {
                GrowSize::Fixed(s) => s.size(parent_size, &mut ChildSize::default(), context),
                GrowSize::Grow(s) => {
                    column_px_per_grow * s.size(parent_size, &mut ChildSize::default(), context)
                }
            };
            let end = start + width;
            column_ranges.push([start, end]);
            current_x = end + column_gap_px;
        }

        let mut row_ranges = Vec::with_capacity(self.template_rows.len());
        let mut current_y = 0.0;
        for size in &self.template_rows {
            let start = current_y;
            let height = match size {
                GrowSize::Fixed(s) => s.size(parent_size, &mut ChildSize::default(), context),
                GrowSize::Grow(s) => {
                    row_px_per_grow * s.size(parent_size, &mut ChildSize::default(), context)
                }
            };
            let end = start + height;
            row_ranges.push([start, end]);
            current_y = end + row_gap_px;
        }

        (column_ranges, row_ranges)
    }
}
