use eframe::egui::{self, Vec2};

/// What to add to [`egui::InputState::smooth_scroll_delta`] for the pointer drag
/// of the current frame, if anything.
///
/// Kept pure so the button press glitch can be unit tested without an egui
/// context.
fn drag_scroll_delta(
    has_touch_screen: bool,
    widget_is_dragged: bool,
    primary_down: bool,
    decidedly_dragging: bool,
    pointer_delta: Vec2,
) -> Vec2 {
    // A touch screen is handled by egui itself (`DragScroll::OnTouch`), adding our
    // delta on top of that would scroll twice as far
    if has_touch_screen {
        return Vec2::ZERO;
    }

    // Sliders, knobs, pedals, `egui_dnd` drag handles, scroll bars, text selection
    // and the pedalboard canvas all own their drags
    if widget_is_dragged {
        return Vec2::ZERO;
    }

    // egui 0.32 handed the scroll area under the pointer the *whole* delta of the
    // frame the pointer went down in (which, with a touch screen, is the distance
    // from wherever the finger last was) and the pointer velocity of the frame it
    // came back up in, even when the press landed on a button. That is what made a
    // list jump when a button in it was pressed. `is_decidedly_dragging` excludes
    // the frame the pointer went down in and `primary_down` excludes the frame it
    // came back up in, so a tap can't move the list and letting go doesn't fling it
    if !primary_down || !decidedly_dragging {
        return Vec2::ZERO;
    }

    // Drag the content around, the same way a `Scene` is panned or the way egui's
    // own drag to scroll works: what gets added is only the movement since the
    // previous frame, so the list follows the finger
    pointer_delta
}

/// Lets the lists in this app be scrolled by dragging them, without the jump a
/// button press used to cause.
///
/// egui 0.36 only turns a drag into scrolling when it can detect a touch screen
/// (`ScrollArea` uses `DragScroll::OnTouch` by default, which asks
/// [`egui::InputState::has_touch_screen`]). That is only aware of `Event::Touch`,
/// so a panel that reports itself as a mouse never gets it, and the lists in this
/// app are then only scrollable with a wheel we don't have. So the drag is fed to
/// the scroll area under the pointer here instead.
///
/// [`egui::InputState::smooth_scroll_delta`] is what a hovered `ScrollArea`
/// consumes, and it only does so for the layer the pointer is actually over, so
/// this can't scroll a list that is behind a window or pan the pedalboard canvas
/// that is behind the pedal menu.
pub fn drag_scroll(ctx: &egui::Context) {
    // Read outside of `input_mut` to not nest two locks on the input state
    let widget_is_dragged = ctx.dragged_id().is_some();

    ctx.input_mut(|input| {
        let delta = drag_scroll_delta(
            input.has_touch_screen(),
            widget_is_dragged,
            input.pointer.primary_down(),
            input.pointer.is_decidedly_dragging(),
            input.pointer.delta(),
        );

        if delta != Vec2::ZERO {
            input.smooth_scroll_delta += delta;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_on_a_button_does_not_move_the_list() {
        // The finger lands 300px away from where it last touched, the panel
        // reports that as a pointer move of the whole distance
        let touch_down = Vec2::new(-300.0, 0.0);
        assert_eq!(
            drag_scroll_delta(false, false, true, false, touch_down),
            Vec2::ZERO
        );

        // Once it is a real drag only the movement of that frame is forwarded
        assert_eq!(
            drag_scroll_delta(false, false, true, true, Vec2::new(0.0, 6.0)),
            Vec2::new(0.0, 6.0)
        );
    }

    #[test]
    fn letting_go_does_not_fling_the_list() {
        // egui 0.32 added `pointer.velocity()` to the scroll offset when the
        // pointer was released, which moved the list again after a button press
        assert_eq!(
            drag_scroll_delta(false, false, false, true, Vec2::new(0.0, 40.0)),
            Vec2::ZERO
        );
    }

    #[test]
    fn dragging_the_list_scrolls_it() {
        assert_eq!(
            drag_scroll_delta(false, false, true, true, Vec2::new(0.0, 12.0)),
            Vec2::new(0.0, 12.0)
        );
    }

    #[test]
    fn dragging_a_widget_does_not_scroll_the_list() {
        // A slider, knob, pedal, drag handle or scroll bar in the list is being
        // dragged: the pointer belongs to it
        assert_eq!(
            drag_scroll_delta(false, true, true, true, Vec2::new(0.0, 12.0)),
            Vec2::ZERO
        );
    }

    #[test]
    fn touch_screens_keep_being_scrolled_by_egui() {
        assert_eq!(
            drag_scroll_delta(true, false, true, true, Vec2::new(0.0, 12.0)),
            Vec2::ZERO
        );
    }

    #[test]
    fn a_tap_then_drag_only_scrolls_by_the_drag_movement() {
        // (pointer down, decidedly dragging, pointer delta this frame)
        let frames = [
            // Touch down, reported as a pointer move from wherever the finger last
            // was. egui 0.32 scrolled the list by this, which is the jump
            (true, false, Vec2::new(-300.0, 0.0)),
            // Dragged past the drag threshold
            (true, true, Vec2::new(0.0, 6.0)),
            (true, true, Vec2::new(0.0, 6.0)),
            // Released while moving, where egui 0.32 added the velocity
            (false, true, Vec2::new(0.0, 40.0)),
        ];

        let scrolled = frames.iter().fold(Vec2::ZERO, |scrolled, &frame| {
            let (primary_down, decidedly_dragging, pointer_delta) = frame;
            scrolled
                + drag_scroll_delta(
                    false,
                    false,
                    primary_down,
                    decidedly_dragging,
                    pointer_delta,
                )
        });

        // Only the 6px drags, none of the 300px touch down or the 40px release
        assert_eq!(scrolled, Vec2::new(0.0, 12.0));
    }
}
