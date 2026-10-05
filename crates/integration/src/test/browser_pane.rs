//! Integration tests for the browser pane.

use warp::integration_testing::pane_group::{assert_focused_pane_index, assert_num_panes_in_tab};
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::integration_testing::view_getters::{pane_group_view, workspace_view};
use warp::pane_group::BrowserPane;
use warp::workspace::WorkspaceAction;
use warpui::{TypedActionView, async_assert};

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
