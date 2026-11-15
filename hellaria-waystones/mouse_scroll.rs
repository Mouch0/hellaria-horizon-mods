// La molette sur l'écran des waystones : page suivante ou précédente.
fn hellaria_waystones_mouse_scroll(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    crate::game::hellaria_waystones::mouse_scrolled(game, screen, scroll, canceled);
}
