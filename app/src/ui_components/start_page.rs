//! Building blocks for the start pages of the browser and preview panes: a heading, a framed input,
//! and sections whose rows sit together on one card.

use warp_core::ui::appearance::Appearance;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Flex,
    Hoverable, MouseStateHandle, ParentElement, Radius, Shrinkable, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::{Action, Element};

use crate::ui_components::icons::Icon;

/// Widest a start page's content grows.
pub const PAGE_WIDTH: f32 = 620.;
const CARD_RADIUS: f32 = 10.;
const ROW_ICON_SIZE: f32 = 14.;
/// The rounded square a row's icon sits in.
const ROW_ICON_TILE: f32 = 26.;
const SECTION_GAP: f32 = 22.;

/// Centers `children` in a column no wider than [`PAGE_WIDTH`], starting near the top.
pub fn page(children: Vec<Box<dyn Element>>) -> Box<dyn Element> {
    Align::new(
        Container::new(
            ConstrainedBox::new(Flex::column().with_children(children).finish())
                .with_max_width(PAGE_WIDTH)
                .finish(),
        )
        .with_margin_top(40.)
        .with_margin_bottom(24.)
        .with_horizontal_padding(20.)
        .finish(),
    )
    .top_center()
    .finish()
}

/// The page's title and, below it, what the page is for.
pub fn heading(title: &str, subtitle: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    Container::new(
        Flex::column()
            .with_spacing(4.)
            .with_child(
                Text::new_inline(
                    title.to_owned(),
                    appearance.ui_font_family(),
                    appearance.ui_font_size() + 7.,
                )
                .with_color(theme.active_ui_text_color().into())
                .with_style(Properties::default().weight(Weight::Semibold))
                .finish(),
            )
            .with_child(
                Text::new(
                    subtitle.to_owned(),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(theme.nonactive_ui_text_color().into())
                .finish(),
            )
            .finish(),
    )
    .with_margin_bottom(18.)
    .finish()
}

/// Frames an input such as a URL field, with `icon` before it.
pub fn input_frame(
    icon: Icon,
    input: Box<dyn Element>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(10.)
            .with_child(
                ConstrainedBox::new(
                    icon.to_warpui_icon(theme.nonactive_ui_text_color())
                        .finish(),
                )
                .with_width(ROW_ICON_SIZE + 2.)
                .with_height(ROW_ICON_SIZE + 2.)
                .finish(),
            )
            .with_child(Shrinkable::new(1., input).finish())
            .finish(),
    )
    .with_horizontal_padding(14.)
    .with_vertical_padding(10.)
    .with_background(theme.surface_1())
    .with_border(Border::all(1.).with_border_fill(theme.outline()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CARD_RADIUS)))
    .finish()
}

/// A titled group of `rows` on one card, separated by thin lines. `extra` follows the card, for a
/// note or link about the section.
pub fn section(
    icon: Icon,
    title: &str,
    rows: Vec<Box<dyn Element>>,
    extra: Option<Box<dyn Element>>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label_color = theme.nonactive_ui_text_color();
    let label = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(7.)
        .with_child(
            ConstrainedBox::new(icon.to_warpui_icon(label_color).finish())
                .with_width(ROW_ICON_SIZE - 1.)
                .with_height(ROW_ICON_SIZE - 1.)
                .finish(),
        )
        .with_child(
            Text::new_inline(
                title.to_owned(),
                appearance.ui_font_family(),
                appearance.ui_font_size() - 1.,
            )
            .with_color(label_color.into())
            .with_style(Properties::default().weight(Weight::Medium))
            .finish(),
        )
        .finish();

    let mut column =
        Flex::column().with_child(Container::new(label).with_margin_bottom(8.).finish());
    if !rows.is_empty() {
        let mut card = Flex::column();
        let count = rows.len();
        for (index, row) in rows.into_iter().enumerate() {
            card.add_child(row);
            if index + 1 < count {
                card.add_child(divider(appearance));
            }
        }
        column.add_child(
            Container::new(card.finish())
                .with_uniform_padding(4.)
                .with_background(theme.surface_1())
                .with_border(Border::all(1.).with_border_fill(theme.outline()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CARD_RADIUS)))
                .finish(),
        );
    }
    if let Some(extra) = extra {
        column.add_child(Container::new(extra).with_margin_top(6.).finish());
    }
    Container::new(column.finish())
        .with_margin_top(SECTION_GAP)
        .finish()
}

fn divider(appearance: &Appearance) -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(
            Container::new(Empty::new().finish())
                .with_background(appearance.theme().outline())
                .finish(),
        )
        .with_height(1.)
        .finish(),
    )
    .with_horizontal_margin(10.)
    .finish()
}

/// What a row shows.
pub struct RowContent<'a> {
    pub icon: Icon,
    pub title: &'a str,
    pub detail: Option<&'a str>,
    /// Shown at the end of the row while it is hovered, such as "Open".
    pub hover_hint: &'static str,
}

/// A clickable row in a section that dispatches `action`.
pub fn row<A: Action + Clone>(
    content: RowContent<'_>,
    mouse_state: &MouseStateHandle,
    action: A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme().clone();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.ui_font_size();
    let RowContent {
        icon,
        title,
        detail,
        hover_hint,
    } = content;
    let title = title.to_owned();
    let detail = detail.map(str::to_owned);
    Hoverable::new(mouse_state.clone(), move |state| {
        let hovered = state.is_hovered();
        let (tile, icon_color) = if hovered {
            (
                theme.accent(),
                theme.font_color(theme.accent().into_solid()),
            )
        } else {
            (theme.surface_2(), theme.nonactive_ui_text_color())
        };
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(12.)
            .with_child(
                ConstrainedBox::new(
                    Container::new(
                        Align::new(
                            ConstrainedBox::new(icon.to_warpui_icon(icon_color).finish())
                                .with_width(ROW_ICON_SIZE)
                                .with_height(ROW_ICON_SIZE)
                                .finish(),
                        )
                        .finish(),
                    )
                    .with_background(tile)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(7.)))
                    .finish(),
                )
                .with_width(ROW_ICON_TILE)
                .with_height(ROW_ICON_TILE)
                .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.,
                    Text::new_inline(title.clone(), font_family, font_size)
                        .with_color(theme.active_ui_text_color().into())
                        .finish(),
                )
                .finish(),
            );
        if let Some(detail) = &detail {
            row.add_child(
                Shrinkable::new(
                    2.,
                    Text::new_inline(detail.clone(), font_family, font_size - 1.)
                        .with_color(theme.nonactive_ui_text_color().into())
                        .finish(),
                )
                .finish(),
            );
        }
        row.add_child(Shrinkable::new(1., Empty::new().finish()).finish());
        if hovered {
            row.add_child(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_spacing(4.)
                    .with_child(
                        Text::new_inline(hover_hint, font_family, font_size - 1.)
                            .with_color(theme.nonactive_ui_text_color().into())
                            .finish(),
                    )
                    .with_child(
                        ConstrainedBox::new(
                            Icon::ArrowRight
                                .to_warpui_icon(theme.nonactive_ui_text_color())
                                .finish(),
                        )
                        .with_width(ROW_ICON_SIZE - 2.)
                        .with_height(ROW_ICON_SIZE - 2.)
                        .finish(),
                    )
                    .finish(),
            );
        }
        let mut container = Container::new(row.finish())
            .with_horizontal_padding(10.)
            .with_vertical_padding(7.)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CARD_RADIUS - 3.)));
        if hovered {
            container = container.with_background(theme.surface_2());
        }
        container.finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
    .finish()
}

/// Muted text, such as a note under a section or an empty state.
pub fn note(text: &str, appearance: &Appearance) -> Box<dyn Element> {
    Container::new(
        Text::new(
            text.to_owned(),
            appearance.ui_font_family(),
            appearance.ui_font_size() - 1.,
        )
        .with_color(appearance.theme().nonactive_ui_text_color().into())
        .finish(),
    )
    .with_horizontal_padding(2.)
    .with_vertical_padding(4.)
    .finish()
}

/// A text button in the accent color that dispatches `action`.
pub fn link<A: Action + Clone>(
    label: &str,
    mouse_state: &MouseStateHandle,
    action: A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme().clone();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.ui_font_size() - 1.;
    let label = label.to_owned();
    Hoverable::new(mouse_state.clone(), move |state| {
        let mut container = Container::new(
            Text::new_inline(label.clone(), font_family, font_size)
                .with_color(theme.accent().into())
                .finish(),
        )
        .with_horizontal_padding(8.)
        .with_vertical_padding(4.)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)));
        if state.is_hovered() {
            container = container.with_background(theme.surface_2());
        }
        container.finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
    .finish()
}
