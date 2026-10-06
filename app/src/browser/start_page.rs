//! Building blocks for the start pages of browser and preview panes: a heading, section labels,
//! tiles in a grid, and cards of rows. Everything takes its colors from the Warp theme.

use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::Fill;
use warpui::assets::asset_cache::AssetSource;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Expanded,
    Flex, Hoverable, Image, MainAxisSize, MouseStateHandle, ParentElement, Radius, Shrinkable,
    Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::image_cache::CacheOption;
use warpui::{Action, Element};

use crate::ui_components::icons::Icon;

/// Widest a start page's content grows.
pub const PAGE_WIDTH: f32 = 920.;
/// Space between tiles in a grid.
pub const GRID_SPACING: f32 = 10.;
const TILE_RADIUS: f32 = 10.;
const BADGE_SIZE: f32 = 28.;
const BADGE_ICON_SIZE: f32 = 14.;
const ROW_ICON_SIZE: f32 = 14.;
const LIVE_DOT_SIZE: f32 = 7.;
const THUMBNAIL_HEIGHT: f32 = 118.;
const HEADING_SCALE: f32 = 1.6;
const HEADING_ICON_SIZE: f32 = 22.;

/// The page's title, in front of the field that starts it.
pub fn page_heading(icon: Icon, text: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    Flex::row()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(10.)
        .with_child(
            ConstrainedBox::new(icon.to_warpui_icon(theme.accent()).finish())
                .with_width(HEADING_ICON_SIZE)
                .with_height(HEADING_ICON_SIZE)
                .finish(),
        )
        .with_child(
            Text::new_inline(
                text.to_owned(),
                appearance.ui_font_family(),
                appearance.ui_font_size() * HEADING_SCALE,
            )
            .with_style(Properties::default().weight(Weight::Semibold))
            .with_color(theme.active_ui_text_color().into())
            .finish(),
        )
        .finish()
}

/// The name of a group of tiles or rows, with a colored dot when `dot` is given.
pub fn section_label(text: &str, dot: Option<Fill>, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(8.);
    if let Some(dot) = dot {
        row.add_child(live_dot(dot));
    }
    row.add_child(
        Text::new_inline(
            text.to_owned(),
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_style(Properties::default().weight(Weight::Semibold))
        .with_color(theme.nonactive_ui_text_color().into())
        .finish(),
    );
    Container::new(row.finish())
        .with_margin_top(26.)
        .with_margin_bottom(10.)
        .finish()
}

/// Lays `tiles` out in rows of `columns`, every tile the same width.
pub fn grid(tiles: Vec<Box<dyn Element>>, columns: usize) -> Box<dyn Element> {
    let columns = columns.max(1);
    let mut column = Flex::column().with_spacing(GRID_SPACING);
    let mut tiles = tiles.into_iter().peekable();
    while tiles.peek().is_some() {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_spacing(GRID_SPACING);
        for _ in 0..columns {
            // Empty cells keep the last row's tiles the same width as the others.
            let tile = tiles.next().unwrap_or_else(|| Empty::new().finish());
            row.add_child(Expanded::new(1., tile).finish());
        }
        column.add_child(row.finish());
    }
    column.finish()
}

/// A tile for something running on this machine, such as a dev server: a badge, a monospace
/// title, a detail line and a live dot.
pub fn server_tile<A: Action + Clone>(
    icon: Icon,
    title: &str,
    detail: &str,
    mouse_state: &MouseStateHandle,
    action: A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme().clone();
    let ui_font = appearance.ui_font_family();
    let mono_font = appearance.monospace_font_family();
    let font_size = appearance.ui_font_size();
    let title = title.to_owned();
    let detail = detail.to_owned();
    Hoverable::new(mouse_state.clone(), move |state| {
        let text = Flex::column()
            .with_spacing(2.)
            .with_child(
                Text::new_inline(title.clone(), mono_font, font_size)
                    .with_color(theme.active_ui_text_color().into())
                    .finish(),
            )
            .with_child(
                Text::new_inline(detail.clone(), ui_font, font_size)
                    .with_color(theme.nonactive_ui_text_color().into())
                    .finish(),
            )
            .finish();
        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(12.)
            .with_child(badge(icon, theme.accent(), theme.surface_2()))
            .with_child(Expanded::new(1., text).finish())
            .with_child(live_dot(Fill::success()))
            .finish();
        tile_container(row, state.is_hovered(), &theme)
            .with_horizontal_padding(14.)
            .with_vertical_padding(10.)
            .finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
    .finish()
}

/// A card for another app's window: its picture when there is one, then the app and the
/// window's title.
pub fn window_card<A: Action + Clone>(
    thumbnail: Option<AssetSource>,
    app_name: &str,
    detail: Option<&str>,
    mouse_state: &MouseStateHandle,
    action: A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme().clone();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.ui_font_size();
    let app_name = app_name.to_owned();
    let detail = detail.map(str::to_owned);
    let initial = app_name
        .chars()
        .next()
        .map(|initial| initial.to_uppercase().to_string())
        .unwrap_or_default();
    Hoverable::new(mouse_state.clone(), move |state| {
        let picture = match &thumbnail {
            Some(asset) => Image::new(asset.clone(), CacheOption::BySize)
                .cover()
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TILE_RADIUS - 4.)))
                .finish(),
            None => Container::new(
                Align::new(
                    ConstrainedBox::new(
                        Icon::Laptop
                            .to_warpui_icon(theme.disabled_ui_text_color())
                            .finish(),
                    )
                    .with_width(HEADING_ICON_SIZE)
                    .with_height(HEADING_ICON_SIZE)
                    .finish(),
                )
                .finish(),
            )
            .with_background(theme.surface_2())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TILE_RADIUS - 4.)))
            .finish(),
        };

        let mut text = Flex::column().with_spacing(2.).with_child(
            Text::new_inline(app_name.clone(), font_family, font_size)
                .with_style(Properties::default().weight(Weight::Semibold))
                .with_color(theme.active_ui_text_color().into())
                .finish(),
        );
        if let Some(detail) = &detail {
            text.add_child(
                Text::new_inline(detail.clone(), font_family, font_size)
                    .with_color(theme.nonactive_ui_text_color().into())
                    .finish(),
            );
        }
        let caption = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(10.)
            .with_child(letter_badge(&initial, font_family, font_size, &theme))
            .with_child(Shrinkable::new(1., text.finish()).finish())
            .finish();

        let body = Flex::column()
            .with_spacing(10.)
            .with_child(
                ConstrainedBox::new(picture)
                    .with_height(THUMBNAIL_HEIGHT)
                    .finish(),
            )
            .with_child(caption)
            .finish();
        tile_container(body, state.is_hovered(), &theme)
            .with_uniform_padding(6.)
            .with_padding_bottom(10.)
            .finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
    .finish()
}

/// A bordered card holding `rows` one above another.
pub fn row_card(rows: Vec<Box<dyn Element>>, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    Container::new(Flex::column().with_children(rows).finish())
        .with_uniform_padding(4.)
        .with_background(theme.surface_1())
        .with_border(Border::all(1.).with_border_fill(theme.outline()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TILE_RADIUS)))
        .finish()
}

/// A clickable row in a [`row_card`]: an icon, a title and, beneath it, a detail line.
pub fn card_row<A: Action + Clone>(
    icon: Icon,
    title: &str,
    detail: Option<&str>,
    mouse_state: &MouseStateHandle,
    action: A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme().clone();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.ui_font_size();
    let title = title.to_owned();
    let detail = detail.map(str::to_owned);
    Hoverable::new(mouse_state.clone(), move |state| {
        let mut text = Flex::column().with_spacing(2.).with_child(
            Text::new_inline(title.clone(), font_family, font_size)
                .with_color(theme.active_ui_text_color().into())
                .finish(),
        );
        if let Some(detail) = &detail {
            text.add_child(
                Text::new_inline(detail.clone(), font_family, font_size)
                    .with_color(theme.nonactive_ui_text_color().into())
                    .finish(),
            );
        }
        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(12.)
            .with_child(
                ConstrainedBox::new(
                    icon.to_warpui_icon(theme.nonactive_ui_text_color())
                        .finish(),
                )
                .with_width(ROW_ICON_SIZE)
                .with_height(ROW_ICON_SIZE)
                .finish(),
            )
            .with_child(Shrinkable::new(1., text.finish()).finish())
            .finish();
        let mut container = Container::new(row)
            .with_horizontal_padding(10.)
            .with_vertical_padding(7.)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TILE_RADIUS - 4.)));
        if state.is_hovered() {
            container = container.with_background(theme.surface_2());
        }
        container.finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
    .finish()
}

/// The frame of a tile: a raised surface with an outline that lights up in the accent color on
/// hover.
fn tile_container(
    child: Box<dyn Element>,
    hovered: bool,
    theme: &warp_core::ui::theme::WarpTheme,
) -> Container {
    let (background, border) = if hovered {
        (theme.surface_2(), theme.accent())
    } else {
        (theme.surface_1(), theme.outline())
    };
    Container::new(child)
        .with_background(background)
        .with_border(Border::all(1.).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TILE_RADIUS)))
}

fn badge(icon: Icon, color: Fill, background: Fill) -> Box<dyn Element> {
    let square = Container::new(
        Align::new(
            ConstrainedBox::new(icon.to_warpui_icon(color).finish())
                .with_width(BADGE_ICON_SIZE)
                .with_height(BADGE_ICON_SIZE)
                .finish(),
        )
        .finish(),
    )
    .with_background(background)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(7.)))
    .finish();
    ConstrainedBox::new(square)
        .with_width(BADGE_SIZE)
        .with_height(BADGE_SIZE)
        .finish()
}

fn letter_badge(
    letter: &str,
    font_family: warpui::fonts::FamilyId,
    font_size: f32,
    theme: &warp_core::ui::theme::WarpTheme,
) -> Box<dyn Element> {
    let background = theme.surface_3();
    let square = Container::new(
        Align::new(
            Text::new_inline(letter.to_owned(), font_family, font_size)
                .with_style(Properties::default().weight(Weight::Semibold))
                .with_color(theme.main_text_color(background).into())
                .finish(),
        )
        .finish(),
    )
    .with_background(background)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(7.)))
    .finish();
    ConstrainedBox::new(square)
        .with_width(BADGE_SIZE)
        .with_height(BADGE_SIZE)
        .finish()
}

fn live_dot(fill: Fill) -> Box<dyn Element> {
    ConstrainedBox::new(
        Container::new(Empty::new().finish())
            .with_background(fill)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(LIVE_DOT_SIZE / 2.)))
            .finish(),
    )
    .with_width(LIVE_DOT_SIZE)
    .with_height(LIVE_DOT_SIZE)
    .finish()
}
