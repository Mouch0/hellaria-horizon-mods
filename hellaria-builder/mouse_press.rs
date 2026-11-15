// Un clic sur l'écran de mélange : curseurs et boutons.
fn hellaria_builder_mouse_press(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::hellaria_builder::mouse_pressed(game, screen, button, canceled);
}
