//! What a preview pane and the picture-in-picture window both show: the front preview, the other
//! previews as a pile of live cards over its corner, and a thin floating bar of controls over the
//! picture.

use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::appearance::Appearance;
use warpui::assets::asset_cache::AssetSource;
use warpui::fonts::FamilyId;
use warpui::elements::{
    Align, Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
    Flex, Hoverable, Image, MainAxisSize, MouseStateHandle,
    OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius, SavePosition,
    Stack, Text,
};
use warpui::scene::DropShadow;
use warpui::image_cache::CacheOption;
use warpui::ui_components::components::UiComponent;
use warpui::{Action, AppContext, Element, SingletonEntity};

use super::streams::{ChromiumState, Preview, PreviewId, PreviewStatus, PreviewStreams};
use crate::browser::AgentApproval;
use crate::ui_components::icons::Icon;

/// Height of the floating control bar.
const BAR_HEIGHT: f32 = 30.;
const BAR_ICON_SIZE: f32 = 15.;
const BAR_BUTTON_WIDTH: f32 = 28.;
const PICTURE_CORNER_RADIUS: f32 = 10.;
/// How many cards the collapsed pile draws; the rest are counted on its badge.
const PILE_DEPTH: usize = 3;
/// How far each card in the collapsed pile sits up and to the left of the one in front of it.
const PILE_STEP: f32 = 7.;
/// Keeps the pile fanned out while the pointer crosses the gaps between cards.
const PILE_HOVER_OUT_DELAY: Duration = Duration::from_millis(250);

/// The stage keeps one dark look in both themes, since it shows another program's pixels.
const STAGE_BACKGROUND: ColorU = ColorU {
    r: 12,
    g: 17,
    b: 16,
    a: 255,
};
const GLASS: ColorU = ColorU {
    r: 26,
    g: 34,
    b: 32,
    a: 220,
};
const GLASS_LINE: ColorU = ColorU {
    r: 255,
    g: 255,
    b: 255,
    a: 44,
};
const GLASS_HOVER: ColorU = ColorU {
    r: 255,
    g: 255,
    b: 255,
    a: 34,
};
const STAGE_TEXT: ColorU = ColorU {
    r: 232,
    g: 239,
    b: 237,
    a: 255,
};
const STAGE_MUTED: ColorU = ColorU {
    r: 169,
    g: 182,
    b: 178,
    a: 255,
};
const DISABLED: ColorU = ColorU {
    r: 169,
    g: 182,
    b: 178,
    a: 110,
};
const LIT: ColorU = ColorU {
    r: 15,
    g: 138,
    b: 121,
    a: 255,
};
const AGENT: ColorU = ColorU {
    r: 240,
    g: 147,
    b: 90,
    a: 255,
};

/// Something the user did on the stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageAction {
    BringToFront(PreviewId),
    Close(PreviewId),
    Retry(PreviewId),
    DownloadChromium,
    GrantScreenRecording,
    Screenshot,
    /// Moves the previews between the pane and the picture-in-picture window.
    TogglePictureInPicture,
    AddPreview,
    ResolveApproval(AgentApproval),
}

/// Mouse state for everything clickable on a stage. Owned by the view showing the stage.
#[derive(Default)]
pub struct StageMouseStates {
    watch: MouseStateHandle,
    control: MouseStateHandle,
    screenshot: MouseStateHandle,
    picture_in_picture: MouseStateHandle,
    add: MouseStateHandle,
    status_button: MouseStateHandle,
    secondary_status_button: MouseStateHandle,
    approve_once: MouseStateHandle,
    approve_always: MouseStateHandle,
    deny: MouseStateHandle,
    pile: MouseStateHandle,
    cards: Vec<(MouseStateHandle, MouseStateHandle)>,
}

impl StageMouseStates {
    /// Makes sure there is mouse state for `count` cards.
    pub fn ensure_cards(&mut self, count: usize) {
        while self.cards.len() < count {
            self.cards.push(Default::default());
        }
    }
}

/// What a stage shows besides its previews.
pub struct StageOptions<'a> {
    pub position_id: &'a str,
    /// Whether the stage is the picture-in-picture window, which changes the pop-out button and
    /// shrinks the pile.
    pub in_picture_in_picture: bool,
    /// An app an agent is waiting for the user to approve.
    pub approval: Option<&'a str>,
    /// What an agent is doing with the front preview right now.
    pub agent_step: Option<&'a str>,
    pub notice: Option<&'a str>,
}

/// Renders `cards` with `front` large. `wrap` turns stage actions into the owning view's actions.
pub fn render_stage<A: Action + Clone>(
    cards: &[PreviewId],
    front: PreviewId,
    mouse_states: &StageMouseStates,
    options: StageOptions<'_>,
    wrap: fn(StageAction) -> A,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let streams = PreviewStreams::as_ref(app);
    let front_preview = streams.get(front);

    let mut picture = Stack::new().with_child(
        // Saved for a single frame so the stage counts as hidden whenever it is not drawn,
        // which pauses its previews.
        SavePosition::new(
            render_picture(front_preview, streams, mouse_states, wrap, appearance),
            options.position_id,
        )
        .for_single_frame()
        .finish(),
    );
    picture.add_positioned_child(
        render_badge(options.agent_step, appearance),
        OffsetPositioning::offset_from_parent(
            vec2f(-12., 12.),
            ParentOffsetBounds::ParentByPosition,
            ParentAnchor::TopRight,
            ChildAnchor::TopRight,
        ),
    );
    picture.add_positioned_child(
        render_bar(&options, mouse_states, wrap, appearance),
        OffsetPositioning::offset_from_parent(
            vec2f(0., -12.),
            ParentOffsetBounds::ParentByPosition,
            ParentAnchor::BottomMiddle,
            ChildAnchor::BottomMiddle,
        ),
    );
    if cards.len() > 1 {
        let size = if options.in_picture_in_picture {
            PileSize::SMALL
        } else {
            PileSize::REGULAR
        };
        picture.add_positioned_child(
            render_pile(cards, front, size, streams, mouse_states, wrap, appearance),
            OffsetPositioning::offset_from_parent(
                vec2f(-12., -12.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::BottomRight,
                ChildAnchor::BottomRight,
            ),
        );
    }
    if let Some(app_name) = options.approval {
        picture.add_positioned_child(
            render_approval(app_name, mouse_states, wrap, appearance),
            OffsetPositioning::offset_from_parent(
                vec2f(0., 12.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::TopMiddle,
                ChildAnchor::TopMiddle,
            ),
        );
    }

    let mut picture = Container::new(picture.finish()).with_uniform_padding(10.);
    if options.agent_step.is_some() {
        picture = picture.with_border(Border::all(2.).with_border_fill(AGENT));
    }
    Container::new(picture.finish())
        .with_background(STAGE_BACKGROUND)
        .finish()
}

fn render_picture<A: Action + Clone>(
    preview: Option<&Preview>,
    streams: &PreviewStreams,
    mouse_states: &StageMouseStates,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let Some(preview) = preview else {
        return Empty::new().finish();
    };
    let message = status_message(preview, streams, mouse_states, wrap, appearance);
    if preview.frame_size.is_none() || message.is_some() {
        let mut stack = Stack::new().with_child(Empty::new().finish());
        if preview.frame_size.is_some() {
            stack.add_child(frame_image(preview, 0.35));
        }
        if let Some(message) = message {
            stack.add_child(Align::new(message).finish());
        }
        return stack.finish();
    }
    frame_image(preview, 1.)
}

fn frame_image(preview: &Preview, opacity: f32) -> Box<dyn Element> {
    frame_image_from(preview.frame_asset(), opacity)
}

fn frame_image_from(asset: AssetSource, opacity: f32) -> Box<dyn Element> {
    Image::new(asset, CacheOption::BySize)
        .contain()
        .with_opacity(opacity)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
            PICTURE_CORNER_RADIUS,
        )))
        .finish()
}

/// What to show over or instead of the picture, when the preview is not simply live.
fn status_message<A: Action + Clone>(
    preview: &Preview,
    streams: &PreviewStreams,
    mouse_states: &StageMouseStates,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Option<Box<dyn Element>> {
    let id = preview.id;
    let (text, buttons): (String, Vec<(&str, StageAction)>) = match &preview.status {
        PreviewStatus::Live => return None,
        PreviewStatus::Starting => (format!("Starting {}…", preview.label()), Vec::new()),
        PreviewStatus::Minimized => (
            "This window is minimized or on another desktop. Restore it to watch it here."
                .to_owned(),
            Vec::new(),
        ),
        PreviewStatus::NeedsPermission => (
            "Warp needs the Screen Recording permission to show other apps' windows. Nothing is \
             recorded while no preview is open."
                .to_owned(),
            vec![("Allow Screen Recording", StageAction::GrantScreenRecording)],
        ),
        PreviewStatus::NeedsChromium => match streams.chromium() {
            ChromiumState::Looking => ("Looking for a browser…".to_owned(), Vec::new()),
            ChromiumState::Downloading => ("Downloading Chromium…".to_owned(), Vec::new()),
            ChromiumState::Failed(reason) => (
                format!("{reason}. Installing Chrome also works."),
                vec![("Try again", StageAction::DownloadChromium)],
            ),
            ChromiumState::Missing | ChromiumState::Found(_) => (
                "Page previews run in a headless Chromium. Warp can download one (about 100 MB), \
                 or use Chrome once it's installed."
                    .to_owned(),
                vec![("Download for me", StageAction::DownloadChromium)],
            ),
        },
        PreviewStatus::Ended(reason) => (
            format!("{reason}."),
            vec![
                ("Try again", StageAction::Retry(id)),
                ("Close", StageAction::Close(id)),
            ],
        ),
    };
    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(12.)
        .with_child(
            ConstrainedBox::new(
                Text::new(text, appearance.ui_font_family(), appearance.ui_font_size())
                    .with_color(STAGE_TEXT)
                    .finish(),
            )
            .with_max_width(360.)
            .finish(),
        );
    if !buttons.is_empty() {
        let handles = [
            &mouse_states.status_button,
            &mouse_states.secondary_status_button,
        ];
        let mut row = Flex::row().with_spacing(8.);
        for (index, (label, action)) in buttons.into_iter().enumerate() {
            row.add_child(pill_button(
                label,
                index == 0,
                handles[index.min(1)],
                wrap(action),
                appearance,
            ));
        }
        column.add_child(row.finish());
    }
    Some(
        Container::new(column.finish())
            .with_uniform_padding(16.)
            .with_background(GLASS)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
            .finish(),
    )
}

/// The "Watching" badge, or what an agent is doing while it acts.
fn render_badge(agent_step: Option<&str>, appearance: &Appearance) -> Box<dyn Element> {
    let (dot, label) = match agent_step {
        Some(step) => (AGENT, step.to_owned()),
        None => (LIT, "Watching".to_owned()),
    };
    Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(6.)
            .with_child(
                ConstrainedBox::new(
                    Container::new(Empty::new().finish())
                        .with_background(dot)
                        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                        .finish(),
                )
                .with_width(6.)
                .with_height(6.)
                .finish(),
            )
            .with_child(
                Text::new_inline(label, appearance.ui_font_family(), 11.5)
                    .with_color(STAGE_TEXT)
                    .finish(),
            )
            .finish(),
    )
    .with_horizontal_padding(10.)
    .with_vertical_padding(3.)
    .with_background(GLASS)
    .with_border(Border::all(1.).with_border_fill(GLASS_LINE))
    .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
    .finish()
}

/// The thin floating bar of icon buttons over the bottom of the picture.
fn render_bar<A: Action + Clone>(
    options: &StageOptions<'_>,
    mouse_states: &StageMouseStates,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let switch = Container::new(
        Flex::row()
            .with_spacing(1.)
            .with_child(bar_button(
                Icon::Eye,
                "Watching: your input stays in Warp",
                BarButtonState::Lit,
                &mouse_states.watch,
                None::<A>,
                appearance,
            ))
            .with_child(bar_button(
                Icon::Hand,
                "Control with pointer and keys: coming next",
                BarButtonState::Disabled,
                &mouse_states.control,
                None::<A>,
                appearance,
            ))
            .finish(),
    )
    .with_uniform_padding(1.)
    .with_background(ColorU::new(0, 0, 0, 97))
    .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
    .finish();

    let (picture_in_picture_icon, picture_in_picture_tip) = if options.in_picture_in_picture {
        (Icon::Maximize, "Back to a pane")
    } else {
        (Icon::Minimize, "Picture in picture")
    };
    let mut bar = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(1.)
        .with_child(switch)
        .with_child(separator())
        .with_child(bar_button(
            Icon::Image,
            "Screenshot: save and copy the picture",
            BarButtonState::Normal,
            &mouse_states.screenshot,
            Some(wrap(StageAction::Screenshot)),
            appearance,
        ))
        .with_child(bar_button(
            picture_in_picture_icon,
            picture_in_picture_tip,
            BarButtonState::Normal,
            &mouse_states.picture_in_picture,
            Some(wrap(StageAction::TogglePictureInPicture)),
            appearance,
        ))
        .with_child(bar_button(
            Icon::Plus,
            "Add a preview",
            BarButtonState::Normal,
            &mouse_states.add,
            Some(wrap(StageAction::AddPreview)),
            appearance,
        ));
    if let Some(notice) = options.notice {
        bar.add_child(
            Container::new(
                Text::new_inline(notice.to_owned(), appearance.ui_font_family(), 11.5)
                    .with_color(STAGE_TEXT)
                    .finish(),
            )
            .with_horizontal_padding(8.)
            .finish(),
        );
    }
    ConstrainedBox::new(
        Container::new(bar.finish())
            .with_uniform_padding(3.)
            .with_background(GLASS)
            .with_border(Border::all(1.).with_border_fill(GLASS_LINE))
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
            .finish(),
    )
    .with_height(BAR_HEIGHT)
    .finish()
}

fn separator() -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(
            Container::new(Empty::new().finish())
                .with_background(GLASS_LINE)
                .finish(),
        )
        .with_width(1.)
        .with_height(14.)
        .finish(),
    )
    .with_horizontal_margin(3.)
    .finish()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BarButtonState {
    Normal,
    Lit,
    Disabled,
}

/// An icon button in the bar. Its tooltip appears above it right away, since the bar has no
/// labels.
fn bar_button<A: Action + Clone>(
    icon: Icon,
    tooltip: &'static str,
    state: BarButtonState,
    mouse_state: &MouseStateHandle,
    action: Option<A>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder().clone();
    let hoverable = Hoverable::new(mouse_state.clone(), move |mouse| {
        let color = match state {
            BarButtonState::Normal | BarButtonState::Lit => STAGE_TEXT,
            BarButtonState::Disabled => DISABLED,
        };
        let background = match state {
            BarButtonState::Lit => Some(LIT),
            BarButtonState::Normal if mouse.is_hovered() => Some(GLASS_HOVER),
            BarButtonState::Normal | BarButtonState::Disabled => None,
        };
        let mut button = Container::new(
            Align::new(
                ConstrainedBox::new(icon.to_warpui_icon(color.into()).finish())
                    .with_width(BAR_ICON_SIZE)
                    .with_height(BAR_ICON_SIZE)
                    .finish(),
            )
            .finish(),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)));
        if let Some(background) = background {
            button = button.with_background(background);
        }
        let button = ConstrainedBox::new(button.finish())
            .with_width(BAR_BUTTON_WIDTH)
            .with_height(BAR_HEIGHT - 8.)
            .finish();
        if !mouse.is_hovered() {
            return button;
        }
        let mut stack = Stack::new().with_child(button);
        stack.add_positioned_overlay_child(
            ui_builder.tool_tip(tooltip.to_owned()).build().finish(),
            OffsetPositioning::offset_from_parent(
                vec2f(0., -9.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::TopMiddle,
                ChildAnchor::BottomMiddle,
            ),
        );
        stack.finish()
    });
    match action {
        Some(action) => hoverable
            .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
            .finish(),
        None => hoverable.finish(),
    }
}

fn pill_button<A: Action + Clone>(
    label: &str,
    primary: bool,
    mouse_state: &MouseStateHandle,
    action: A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = label.to_owned();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.ui_font_size();
    Hoverable::new(mouse_state.clone(), move |mouse| {
        let background = match (primary, mouse.is_hovered()) {
            (true, _) => LIT,
            (false, true) => GLASS_HOVER,
            (false, false) => ColorU::transparent_black(),
        };
        Container::new(
            Text::new_inline(label.clone(), font_family, font_size)
                .with_color(STAGE_TEXT)
                .finish(),
        )
        .with_horizontal_padding(12.)
        .with_vertical_padding(4.)
        .with_background(background)
        .with_border(Border::all(1.).with_border_fill(if primary { LIT } else { GLASS_LINE }))
        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
        .finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
    .finish()
}

/// Asks the user whether an agent may watch an app, with the browser pane's three answers.
fn render_approval<A: Action + Clone>(
    app_name: &str,
    mouse_states: &StageMouseStates,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let text = format!(
        "An agent wants to watch {app_name}. It will see this window and its log, but can't \
         click or type in it."
    );
    Container::new(
        Flex::column()
            .with_spacing(10.)
            .with_child(
                ConstrainedBox::new(
                    Text::new(text, appearance.ui_font_family(), appearance.ui_font_size())
                        .with_color(STAGE_TEXT)
                        .finish(),
                )
                .with_max_width(380.)
                .finish(),
            )
            .with_child(
                Flex::row()
                    .with_spacing(6.)
                    .with_child(pill_button(
                        "Allow once",
                        true,
                        &mouse_states.approve_once,
                        wrap(StageAction::ResolveApproval(AgentApproval::Once)),
                        appearance,
                    ))
                    .with_child(pill_button(
                        "Always allow",
                        false,
                        &mouse_states.approve_always,
                        wrap(StageAction::ResolveApproval(AgentApproval::Always)),
                        appearance,
                    ))
                    .with_child(pill_button(
                        "Deny",
                        false,
                        &mouse_states.deny,
                        wrap(StageAction::ResolveApproval(AgentApproval::Deny)),
                        appearance,
                    ))
                    .finish(),
            )
            .finish(),
    )
    .with_uniform_padding(14.)
    .with_background(GLASS)
    .with_border(Border::all(1.).with_border_fill(GLASS_LINE))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
    .finish()
}

/// Sizes of the pile's cards: collapsed thumbnails and fanned-out cards.
#[derive(Clone, Copy)]
struct PileSize {
    thumbnail: (f32, f32),
    card: (f32, f32),
}

impl PileSize {
    const REGULAR: Self = Self {
        thumbnail: (112., 63.),
        card: (152., 86.),
    };
    const SMALL: Self = Self {
        thumbnail: (72., 41.),
        card: (104., 59.),
    };
}

/// The previews behind the front one, piled up over the picture's corner. Hovering the pile fans
/// it out into a row of cards; clicking a card brings it to the front.
fn render_pile<A: Action + Clone>(
    cards: &[PreviewId],
    front: PreviewId,
    size: PileSize,
    streams: &PreviewStreams,
    mouse_states: &StageMouseStates,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let behind: Vec<(&Preview, &(MouseStateHandle, MouseStateHandle))> = cards
        .iter()
        .zip(&mouse_states.cards)
        .filter(|(id, _)| **id != front)
        .filter_map(|(id, states)| Some((streams.get(*id)?, states)))
        .collect();
    if behind.is_empty() {
        return Empty::new().finish();
    }

    let font_family = appearance.ui_font_family();
    Hoverable::new(mouse_states.pile.clone(), |mouse| {
        if mouse.is_hovered() {
            return glass_tray(
                Flex::row()
                    .with_spacing(6.)
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_children(behind.iter().map(|(preview, (card_state, close_state))| {
                        render_card(preview, size, card_state, close_state, wrap, appearance)
                    }))
                    .finish(),
            );
        }
        let thumbnails: Vec<Option<AssetSource>> = behind
            .iter()
            .skip(behind.len().saturating_sub(PILE_DEPTH))
            .map(|(preview, _)| preview.frame_size.map(|_| preview.frame_asset()))
            .collect();
        let hidden_count = behind.len().saturating_sub(PILE_DEPTH);
        render_collapsed_pile(&thumbnails, hidden_count, size, font_family)
    })
    .with_hover_out_delay(PILE_HOVER_OUT_DELAY)
    .finish()
}

/// Up to [`PILE_DEPTH`] thumbnails stacked with a small offset, the newest in front.
fn render_collapsed_pile(
    thumbnails: &[Option<AssetSource>],
    hidden_count: usize,
    size: PileSize,
    font_family: FamilyId,
) -> Box<dyn Element> {
    let (width, height) = size.thumbnail;
    let depth = thumbnails.len().saturating_sub(1) as f32 * PILE_STEP;
    let mut pile = Stack::new().with_child(
        ConstrainedBox::new(Empty::new().finish())
            .with_width(width + depth)
            .with_height(height + depth)
            .finish(),
    );
    for (index, asset) in thumbnails.iter().enumerate() {
        let picture = match asset {
            Some(asset) => frame_image_from(asset.clone(), 1.),
            None => Empty::new().finish(),
        };
        let thumbnail = Container::new(
            ConstrainedBox::new(picture)
                .with_width(width)
                .with_height(height)
                .finish(),
        )
        .with_background(STAGE_BACKGROUND)
        .with_border(Border::all(1.).with_border_fill(GLASS_LINE))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .with_drop_shadow(DropShadow::default())
        .finish();
        let offset = index as f32 * PILE_STEP;
        pile.add_positioned_child(
            thumbnail,
            OffsetPositioning::offset_from_parent(
                vec2f(offset, offset),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::TopLeft,
                ChildAnchor::TopLeft,
            ),
        );
    }
    if hidden_count > 0 {
        pile.add_positioned_child(
            Container::new(
                Text::new_inline(format!("+{hidden_count}"), font_family, 11.)
                    .with_color(STAGE_TEXT)
                    .finish(),
            )
            .with_horizontal_padding(6.)
            .with_vertical_padding(1.)
            .with_background(GLASS)
            .with_border(Border::all(1.).with_border_fill(GLASS_LINE))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .finish(),
            OffsetPositioning::offset_from_parent(
                vec2f(-4., -4.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::BottomRight,
                ChildAnchor::BottomRight,
            ),
        );
    }
    pile.finish()
}

fn glass_tray(child: Box<dyn Element>) -> Box<dyn Element> {
    Container::new(child)
        .with_uniform_padding(6.)
        .with_background(GLASS)
        .with_border(Border::all(1.).with_border_fill(GLASS_LINE))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
        .with_drop_shadow(DropShadow::default())
        .finish()
}

fn render_card<A: Action + Clone>(
    preview: &Preview,
    size: PileSize,
    card_state: &MouseStateHandle,
    close_state: &MouseStateHandle,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let id = preview.id;
    let label = preview.label();
    let asset = preview.frame_size.map(|_| preview.frame_asset());
    let font_family = appearance.ui_font_family();
    let (width, height) = size.card;
    let card = Hoverable::new(card_state.clone(), move |mouse| {
        let picture = match &asset {
            Some(asset) => frame_image_from(asset.clone(), 1.),
            None => Empty::new().finish(),
        };
        let (border, text) = if mouse.is_hovered() {
            (LIT, STAGE_TEXT)
        } else {
            (GLASS_LINE, STAGE_MUTED)
        };
        ConstrainedBox::new(
            Container::new(
                Flex::column()
                    .with_spacing(4.)
                    .with_child(ConstrainedBox::new(picture).with_height(height).finish())
                    .with_child(
                        Text::new_inline(label.clone(), font_family, 11.)
                            .with_color(text)
                            .finish(),
                    )
                    .finish(),
            )
            .with_uniform_padding(4.)
            .with_border(Border::all(1.).with_border_fill(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .finish(),
        )
        .with_width(width)
        .finish()
    })
    .on_click(move |ctx, _, _| ctx.dispatch_typed_action(wrap(StageAction::BringToFront(id))))
    .finish();

    let mut stack = Stack::new().with_child(card);
    stack.add_positioned_child(
        bar_button(
            Icon::X,
            "Close this preview",
            BarButtonState::Normal,
            close_state,
            Some(wrap(StageAction::Close(id))),
            appearance,
        ),
        OffsetPositioning::offset_from_parent(
            vec2f(-4., 4.),
            ParentOffsetBounds::ParentByPosition,
            ParentAnchor::TopRight,
            ChildAnchor::TopRight,
        ),
    );
    stack.finish()
}
