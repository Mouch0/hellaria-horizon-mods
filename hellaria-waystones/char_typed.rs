// Un caractère tapé sur l'écran des waystones : la recherche.
fn hellaria_waystones_char_typed(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    crate::game::hellaria_waystones::char_typed(game, c, canceled);
}
