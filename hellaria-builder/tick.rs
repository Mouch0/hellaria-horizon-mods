// Chaque tick du client : annonce du canal, actions laissées par l'écran.
fn hellaria_builder_tick(game: &mut crate::game::Game<'_>) {
    crate::game::hellaria_builder::tick(game);
}
