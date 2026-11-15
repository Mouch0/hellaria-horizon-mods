// Une touche sur l'écran des waystones : Retour arrière, pages.
fn hellaria_waystones_key_press(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    key: &str,
    canceled: &mut bool,
) {
    crate::game::hellaria_waystones::key_pressed(game, screen, key, canceled);
}
