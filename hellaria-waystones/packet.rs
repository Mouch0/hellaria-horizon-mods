// Un paquet du serveur : la liste des destinations d'une waystone.
fn hellaria_waystones_packet(packet: &horizon_protocol::packets::play::Clientbound) {
    crate::game::hellaria_waystones::client_packet(packet);
}
