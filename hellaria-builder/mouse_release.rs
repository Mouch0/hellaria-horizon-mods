// Le bouton relâché : fin du glissement d'un curseur.
fn hellaria_builder_mouse_release(
    game: &crate::game::Game<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::hellaria_builder::mouse_released(game, button, canceled);
}
