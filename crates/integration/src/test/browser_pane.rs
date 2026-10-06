//! Integration tests for the browser pane.

use warp::BrowserViewRegistry;
use warp::features::FeatureFlag;
use warp::integration_testing::pane_group::{
    assert_focused_pane_index, assert_num_panes_in_tab, close_pane_by_index,
};
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::integration_testing::view_getters::{pane_group_view, workspace_view};
use warp::pane_group::BrowserPane;
use warp::workspace::WorkspaceAction;
use warpui_core::{SingletonEntity, TypedActionView, async_assert};

use crate::Builder;

pub fn test_open_browser_pane_splits_and_focuses() -> Builder {
    Builder::new()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Open a browser pane")
                .with_action(|app, window_id, _| {
                    workspace_view(app, window_id).update(app, |workspace, ctx| {
                        workspace
                            .handle_action(&WorkspaceAction::OpenBrowserPane { url: None }, ctx);
                    });
                })
                .add_named_assertion(
                    "tab splits into the terminal and a new pane",
                    assert_num_panes_in_tab(0, 2),
                )
                .add_named_assertion("new pane has focus", assert_focused_pane_index(0, 1))
                .add_named_assertion("new pane is a browser pane", |app, window_id| {
                    pane_group_view(app, window_id, 0).read(app, |pane_group, _| {
                        let is_browser_pane = pane_group
                            .pane_id_from_index(1)
                            .and_then(|pane_id| {
                                pane_group.downcast_pane_by_id::<BrowserPane>(pane_id)
                            })
                            .is_some();
                        async_assert!(is_browser_pane, "pane 1 should be a browser pane")
                    })
                }),
        )
}

/// A closed pane stays alive, hidden, while its close can be undone. Agents must not be handed its
/// tabs, or they would act in a pane the user cannot see.
pub fn test_closed_browser_pane_is_not_offered_to_agents() -> Builder {
    FeatureFlag::UndoClosedPanes.set_enabled(true);
    Builder::new()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Open a browser pane")
                .with_action(|app, window_id, _| {
                    workspace_view(app, window_id).update(app, |workspace, ctx| {
                        workspace
                            .handle_action(&WorkspaceAction::OpenBrowserPane { url: None }, ctx);
                    });
                })
                .add_named_assertion("agents see the new browser tab", |app, _| {
                    let tabs = app.read(|ctx| BrowserViewRegistry::as_ref(ctx).tabs(ctx).len());
                    async_assert!(tabs == 1, "expected 1 browser tab, found {tabs}")
                }),
        )
        .with_step(close_pane_by_index(0, 1))
        .with_step(
            new_step_with_default_assertions("Closed browser pane is hidden from agents")
                .add_named_assertion("only the terminal pane is visible", |app, window_id| {
                    pane_group_view(app, window_id, 0).read(app, |pane_group, _| {
                        let visible = pane_group.visible_pane_count();
                        async_assert!(visible == 1, "expected 1 visible pane, found {visible}")
                    })
                })
                .add_named_assertion("agents see no browser tabs", |app, _| {
                    let (tabs, resolved) = app.read(|ctx| {
                        let registry = BrowserViewRegistry::as_ref(ctx);
                        (registry.tabs(ctx).len(), registry.resolve(None, ctx).is_some())
                    });
                    async_assert!(
                        tabs == 0 && !resolved,
                        "expected no browser tabs for agents, found {tabs} (current resolves: {resolved})"
                    )
                }),
        )
}
