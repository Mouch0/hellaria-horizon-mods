// Un paquet du serveur : l'état de la baguette tenue.
fn hellaria_builder_packet(packet: &horizon_protocol::packets::play::Clientbound) {
    crate::game::hellaria_builder::client_packet(packet);
}
