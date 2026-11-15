// Le bouton relâché : l'écran des waystones le garde pour lui.
fn hellaria_waystones_mouse_release(game: &crate::game::Game<'_>, canceled: &mut bool) {
    crate::game::hellaria_waystones::mouse_released(game, canceled);
}
