# Companion Bindings

A **companion control** is a strip, menu or bar that edits another control's state without
owning the document. `ColumnChooser`, `HeaderMenu`, `GridStatusBar`, `SummaryRow`,
`TreeFindBar`, `Breadcrumb` and `FindReplaceBar` are companions.

The binding shape is the same for every pair, and it is the template to copy when a new
companion is added.

## The three steps

1. **The companion exposes a source trait or a signal.** `TreeFindBar` consumes a
   `TreeSearchSource`; `FindReplaceBar` reports its intents through signals; `HeaderMenu`
   reports sort, filter and hide requests through signals. The companion never holds the
   document.
2. **The host adds one `attach_*` method.** It subscribes with a `ConnectionScope`, resolves
   the companion's tokens against its own columns/rows *at attach time*, and parks requests in a
   shared queue.
3. **The host polls once per frame.** `poll_*` drains the queue and applies it to its own state,
   so a companion click reaches the host without the application forwarding anything by hand.

## Attach methods by host

| Host | Attach | Poll |
|---|---|---|
| `TreeView` | `attach_tree_find_bar`, `attach_path_bar` | `poll_tree_find_bar`, `poll_path_bar` |
| `TreeTable` | `attach_tree_find_bar`, `attach_column_chooser`, `attach_header_menu`, `attach_grid_status_bar`, `attach_summary_row`, `attach_path_bar` | `poll_tree_find_bar`, `poll_column_chooser`, `poll_header_menu`, `poll_grid_status_bar`, `poll_summary_row`, `poll_path_bar` |
| `GridTable` | `attach_column_chooser`, `attach_header_menu`, `attach_grid_status_bar`, `attach_summary_row` | `poll_column_chooser`, `poll_header_menu`, `poll_grid_status_bar`, `poll_summary_row` |
| `DataGrid` | `attach_column_chooser`, `attach_header_menu`, `attach_grid_status_bar`, `attach_summary_row`, `attach_find_bar` | the matching `poll_*` / `poll_find_bar` |
| `RichEdit` | `attach_find_replace_bar` | `poll_find_replace_bar` |

## Rules that keep the binding honest

- **Resolve tokens at attach time.** A token the host does not know is ignored, never guessed.
- **One subscription per companion.** Attaching replaces the previous binding, so calling
  `attach_*` twice leaves one link rather than two.
- **Support only what the host can honour.** `TreeTable` and `GridTable` ignore a
  `HeaderMenu` filter request: they have no filter model, and reporting one would describe a
  state the picture does not have.
- **Detach keeps the last state.** `detach_*` drops the subscription; the sort, visibility or
  text the companion last set stays until the host changes it.

A worked example is the chooser link in `TreeTable::attach_column_chooser` plus
`TreeTable::poll_column_chooser`; the header menu link follows the same shape with a request
queue.
