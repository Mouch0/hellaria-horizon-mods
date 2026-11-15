// Chaque image : les contours des blocs que la baguette poserait.
fn hellaria_builder_preview(
    game: &crate::game::Game<'_>,
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::hellaria_builder::world_primitives(game, primitives);
}
