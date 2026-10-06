CREATE TABLE browser_panes (
  id INTEGER PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL DEFAULT 'browser' CHECK (kind = 'browser'),

  -- JSON array of the pane's tab URLs, in tab strip order. An empty URL is a new-tab page.
  tab_urls TEXT NOT NULL,

  -- Index into tab_urls of the active tab.
  active_tab_index INTEGER NOT NULL,

  FOREIGN KEY (id, kind) REFERENCES pane_leaves (pane_node_id, kind)
);
