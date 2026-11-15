// Un clic sur l'écran des waystones : destination ou page.
fn hellaria_waystones_mouse_press(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::hellaria_waystones::mouse_pressed(game, screen, button, canceled);
}
