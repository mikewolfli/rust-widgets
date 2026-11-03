# Presentation-only controls

> Upstream feedback: `docs/issues/feedback-20261103.md` RW-104.
> This page answers "is an empty `events` / `commands` list intentional?" once, so a consumer
> does not have to re-judge it every round.

A control with no events and no commands is a legitimate shape: it paints a value or allocates
space and never changes that value. The lists below are also **pinned by a test**
(`capability::properties_tests::presentation_only_controls_are_the_declared_set`), so a control
that gains or loses a list fails the test until the difference is classified. A new control can
therefore not land with an accidental empty surface.

## Controls that publish no events

These render a fixed or caller-supplied value and have no state transition to announce.

| Control | Why there is nothing to announce |
|---|---|
| `align` | Aligns its child; the child owns every event. |
| `aspect_ratio` | Constrains its child's box. |
| `center` | Centres its child. |
| `divider` | A decorative rule. |
| `drawer_section_header` | A static group caption inside a drawer. |
| `expanded` | A flex spacer that expands. |
| `flexible` | A flex slot with a flex factor. |
| `font_preview` | Paints a sample of a font. |
| `frame` | A decorative border/panel container. |
| `group_avatar` | A stacked avatar cluster; members are separate controls. |
| `icon` | A painted glyph. |
| `ime_preedit` | Shows the IME composition string; the enclosing editor owns the events. |
| `intrinsic_width` | Reports the child's intrinsic width. |
| `line` | A painted line. |
| `padding` | Adds inset space around its child. |
| `qr_code` | Paints an encoded matrix. |
| `safe_area` | Insets its child by the platform safe area. |
| `sized_box` | A fixed-size box. |
| `skeleton_loader` | A placeholder animation with no user interaction. |
| `spacer` | Empty space. |
| `tooltip` | Shown transiently by its owner; the owner's signals carry the events. |

## Controls that publish no commands

These have no operation a design document could invoke beyond the `set_*` property route.

| Control | Why there is nothing to invoke |
|---|---|
| `color_history` | The recent-colour list is read-only content; the owning picker performs the selection. |
| `group_avatar` | Members are separate controls; the cluster itself has no operation. |
| `intrinsic_width` | Reports a measured width; there is nothing to change. |
| `padding` | Its inset is a property, not an operation. |
| `sized_box` | Its size is a property, not an operation. |

## Formerly in the list

`swipe_to_dismiss` and `axis_picker` were flagged by RW-104 as suspected gaps and **are** gaps:
both had programmatic operations that no design document could call. They now publish commands.

| Control | Published commands | Dispatch |
|---|---|---|
| `swipe_to_dismiss` | `dismiss`, `reset_swipe` | `WidgetProperties::command` delegates to `SwipeToDismiss::dismiss` / `reset_swipe`, so a wire and a host call share the signal, the animation target and the redraw. |
| `axis_picker` | `set_series_names`, `set_assignments`, `set_edges` | Resolved by the `set_*` convention against the control's own property list, the same route every other data-bearing control uses. |

The gesture (swipe left/right) stays the primary path for `swipe_to_dismiss`; the commands make the
same two facts reachable from a document.
