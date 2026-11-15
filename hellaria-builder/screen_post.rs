// L'écran de mélange, dessiné par-dessus l'écran du mod.
fn hellaria_builder_screen_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    crate::game::hellaria_builder::screen_render_post(game, batch, screen);
}
