//! What a preview pane and the picture-in-picture window both show: the front preview, the other
//! previews as live cards beside it, and a thin floating bar of controls over the picture.

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::appearance::Appearance;
use warpui::assets::asset_cache::AssetSource;
use warpui::elements::{
    Align, Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty,
    Expanded, Flex, Hoverable, Image, MainAxisAlignment, MainAxisSize, MouseStateHandle,
    OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius, SavePosition,
    Stack, Text,
};
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
/// Width of the column of cards beside the front preview.
const CARD_WIDTH: f32 = 168.;
const CARD_PICTURE_HEIGHT: f32 = 94.;
const PICTURE_CORNER_RADIUS: f32 = 10.;

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
    /// Whether the stage is the picture-in-picture window, which changes the pop-out button.
    pub in_picture_in_picture: bool,
    /// Whether the previews behind the front one show as cards beside it.
    pub show_cards: bool,
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
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(Expanded::new(1., picture.finish()).finish());
    if options.show_cards && cards.len() > 1 {
        row.add_child(render_cards(
            cards,
            front,
            streams,
            mouse_states,
            wrap,
            appearance,
        ));
    }
    Container::new(row.finish())
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

/// The previews behind the front one, as small live cards. Clicking one brings it to the front.
fn render_cards<A: Action + Clone>(
    cards: &[PreviewId],
    front: PreviewId,
    streams: &PreviewStreams,
    mouse_states: &StageMouseStates,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let mut column = Flex::column()
        .with_spacing(8.)
        .with_main_axis_size(MainAxisSize::Min)
        .with_main_axis_alignment(MainAxisAlignment::Start);
    for (id, (card_state, close_state)) in cards.iter().zip(&mouse_states.cards) {
        let Some(preview) = streams.get(*id) else {
            continue;
        };
        column.add_child(render_card(
            preview,
            *id == front,
            card_state,
            close_state,
            wrap,
            appearance,
        ));
    }
    ConstrainedBox::new(
        Container::new(column.finish())
            .with_uniform_padding(10.)
            .with_border(Border::left(1.).with_border_fill(GLASS_LINE))
            .finish(),
    )
    .with_width(CARD_WIDTH + 20.)
    .finish()
}

fn render_card<A: Action + Clone>(
    preview: &Preview,
    is_front: bool,
    card_state: &MouseStateHandle,
    close_state: &MouseStateHandle,
    wrap: fn(StageAction) -> A,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let id = preview.id;
    let label = preview.label();
    let asset = preview.frame_size.map(|_| preview.frame_asset());
    let font_family = appearance.ui_font_family();
    let card = Hoverable::new(card_state.clone(), move |mouse| {
        let picture = match &asset {
            Some(asset) => frame_image_from(asset.clone(), 1.),
            None => Empty::new().finish(),
        };
        let border = if is_front || mouse.is_hovered() {
            LIT
        } else {
            GLASS_LINE
        };
        Container::new(
            Flex::column()
                .with_spacing(4.)
                .with_child(
                    ConstrainedBox::new(picture)
                        .with_height(CARD_PICTURE_HEIGHT)
                        .finish(),
                )
                .with_child(
                    Text::new_inline(label.clone(), font_family, 11.5)
                        .with_color(if is_front { STAGE_TEXT } else { STAGE_MUTED })
                        .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(4.)
        .with_border(Border::all(1.).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
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
