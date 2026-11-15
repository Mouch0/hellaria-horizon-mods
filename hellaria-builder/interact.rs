// Maj + clic avec la baguette : orientation (gauche) ou écran de mélange (droit).
fn hellaria_builder_interact(
    game: &mut crate::game::Game<'_>,
    button: i32,
    swing_hand: &mut bool,
    canceled: &mut bool,
) {
    crate::game::hellaria_builder::interaction_key(game, button, swing_hand, canceled);
}
