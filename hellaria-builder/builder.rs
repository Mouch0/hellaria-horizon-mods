// Hellaria Builder Wand pour Horizon : l'aperçu des blocs que la baguette va poser, l'écran de
// mélange du sac et le basculement horizontal / vertical, comme le mod Java côté client.
//
// Le serveur (hellaria-bridge) envoie l'état de la baguette sur `hellaria_builder:hz_state` dès que
// le client a annoncé ce canal ; les réglages repartent sur les canaux du mod Java lui-même
// (`hellaria_builder:toggle`, `hellaria_builder:set_weights`).
use crate::game::Game;
use crate::game::native_hooks::NativeScreenView;
use horizon_client::hud::HudBatch;
use horizon_client::world_primitives::WorldPrimitives;
use horizon_protocol::packets::configuration::CustomPayload;
use horizon_protocol::packets::play::{Clientbound, Serverbound};
use std::cell::RefCell;

pub(super) const MOD_ID: &str = "hellaria_builder";
const STATE_CHANNEL: &str = "hellaria_builder:hz_state";
const TOGGLE_CHANNEL: &str = "hellaria_builder:toggle";
const WEIGHTS_CHANNEL: &str = "hellaria_builder:set_weights";
const SLOTS: usize = 9;

/// Ce que le serveur dit de la baguette tenue.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct WandState {
    /// 3 ou 5 (côté de la zone posée), 0 sans baguette en main.
    pub(super) size: i32,
    pub(super) vertical: bool,
    pub(super) bag: bool,
    /// Blocs disponibles dans le sac (curseurs à 0 exclus).
    pub(super) available: u64,
    pub(super) slots: Vec<Slot>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Slot {
    pub(super) weight: i32,
    pub(super) count: u32,
    /// L'objet tel que ce client le connaît, et son modèle (`minecraft:item_model`).
    pub(super) item: String,
    pub(super) model: String,
    pub(super) name: String,
}

/// Ce qu'un hook qui n'a que `&Game` laisse au tick suivant.
enum Action {
    Toggle,
    Apply(Vec<i32>),
    Close,
}

#[derive(Default)]
struct Screen {
    weights: Vec<i32>,
    /// Le curseur tenu par la souris.
    dragging: Option<usize>,
}

#[derive(Default)]
struct State {
    registered: bool,
    wand: WandState,
    screen: Option<Screen>,
    pending: Vec<Action>,
    toggle_cooldown: u32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// ---------------------------------------------------------------- état reçu

/// Lit l'état texte envoyé par le serveur (voir BuilderBridge.state).
pub(super) fn parse(text: &str) -> Option<WandState> {
    let mut lines = text.lines();
    if lines.next()? != "hb1" {
        return None;
    }
    let mut st = WandState::default();
    for line in lines {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "wand" => st.size = value.trim().parse().unwrap_or(0),
            "vertical" => st.vertical = value.trim() == "1",
            "bag" => st.bag = value.trim() == "1",
            "available" => st.available = value.trim().parse().unwrap_or(0),
            "slot" => {
                let mut parts = value.splitn(5, '|');
                let weight = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                let count = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                let item = parts.next().unwrap_or("").to_owned();
                let model = parts.next().unwrap_or("").to_owned();
                let name = parts.next().unwrap_or("").to_owned();
                st.slots.push(Slot { weight, count, item, model, name });
            }
            _ => {}
        }
    }
    Some(st)
}

/// Les poids en paquet `set_weights` : leur nombre puis chacun, en VarInt (ByteBufCodecs.list).
pub(super) fn encode_weights(weights: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    write_varint(&mut out, weights.len() as i32);
    for &w in weights {
        write_varint(&mut out, w);
    }
    out
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7F == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7F) | 0x80);
        v >>= 7;
    }
}

fn send(game: &mut Game<'_>, channel: &str, data: Vec<u8>) -> bool {
    game.native_send_packet(Serverbound::CustomPayload(CustomPayload {
        channel: channel.into(),
        data,
    }))
    .is_ok()
}

// ---------------------------------------------------------------- hooks

/// À chaque tick du client : annonce le canal au serveur une fois par connexion, et fait ce que
/// les hooks d'écran ont laissé.
pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_on(MOD_ID) {
        return;
    }
    if game.native_player().is_none() {
        with(|s| {
            s.registered = false;
            s.wand = WandState::default();
            s.screen = None;
            s.pending.clear();
        });
        return;
    }
    if !with(|s| s.registered) {
        // minecraft:register : les canaux que ce client écoute, séparés par un octet nul.
        let ok = send(game, "minecraft:register", STATE_CHANNEL.as_bytes().to_vec());
        with(|s| s.registered = ok);
    }
    let pending = with(|s| {
        s.toggle_cooldown = s.toggle_cooldown.saturating_sub(1);
        std::mem::take(&mut s.pending)
    });
    for action in pending {
        match action {
            Action::Toggle => {
                send(game, TOGGLE_CHANNEL, Vec::new());
            }
            Action::Apply(weights) => {
                send(game, WEIGHTS_CHANNEL, encode_weights(&weights));
                game.native_mod_screen_close(MOD_ID);
            }
            Action::Close => game.native_mod_screen_close(MOD_ID),
        }
    }
    // L'écran a été fermé (Échap) : on oublie ses réglages non appliqués.
    if game.native_mod_screen() != Some(MOD_ID) {
        with(|s| s.screen = None);
    }
}

/// Un paquet du serveur : l'état de la baguette.
pub(super) fn client_packet(packet: &Clientbound) {
    if let Clientbound::CustomPayload(p) = packet
        && p.channel == STATE_CHANNEL
        && let Ok(text) = std::str::from_utf8(&p.data)
        && let Some(st) = parse(text)
    {
        with(|s| s.wand = st);
    }
}

/// Maj + clic gauche : orientation ; Maj + clic droit : écran de mélange. Annule l'action vanilla.
pub(super) fn interaction_key(
    game: &mut Game<'_>,
    button: i32,
    swing_hand: &mut bool,
    canceled: &mut bool,
) {
    if !game.native_on(MOD_ID) || *canceled {
        return;
    }
    let size = with(|s| s.wand.size);
    if size == 0 || !game.native_sneaking() {
        return;
    }
    match button {
        0 => {
            *canceled = true;
            *swing_hand = false;
            with(|s| {
                if s.toggle_cooldown == 0 {
                    s.pending.push(Action::Toggle);
                    s.toggle_cooldown = 6;
                }
            });
        }
        1 => {
            *canceled = true;
            *swing_hand = false;
            if game.native_mod_screen_open(MOD_ID) {
                with(|s| {
                    let mut weights: Vec<i32> = s.wand.slots.iter().map(|sl| sl.weight).collect();
                    weights.resize(SLOTS, 50);
                    s.screen = Some(Screen { weights, dragging: None });
                });
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- aperçu

/// Direction : son vecteur et son ordinal Minecraft (DOWN, UP, NORTH, SOUTH, WEST, EAST).
fn ordinal(face: [i32; 3]) -> u32 {
    match face {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        _ => 5,
    }
}

/// 0 = X, 1 = Y, 2 = Z.
fn axis(face: [i32; 3]) -> usize {
    if face[0] != 0 {
        0
    } else if face[1] != 0 {
        1
    } else {
        2
    }
}

/// L'axe horizontal du regard (`Player.getDirection().getAxis()`), lacet en degrés (0 = sud).
fn facing_axis(yaw: f32) -> usize {
    let quarter = ((yaw / 90.0) + 0.5).floor() as i32 & 3;
    if quarter % 2 == 0 { 2 } else { 0 }
}

/// Les cases visées, dans l'ordre où le serveur les remplit (Placement.computeTargets), avant le
/// tri de ce qui est posable.
pub(super) fn candidates(
    pos: [i32; 3],
    face: [i32; 3],
    size: i32,
    vertical: bool,
    yaw: f32,
) -> (Vec<[i32; 3]>, bool) {
    let face_axis = axis(face);
    let plane = if !vertical {
        1
    } else if face_axis != 1 {
        face_axis
    } else {
        facing_axis(yaw)
    };
    let flush = plane == face_axis;
    let half = size / 2;
    let step = if flush { 1 } else { 1 + half };
    let center = [
        pos[0] + face[0] * step,
        pos[1] + face[1] * step,
        pos[2] + face[2] * step,
    ];
    let mut out = Vec::with_capacity((size * size) as usize);
    for a in -half..=half {
        for b in -half..=half {
            let d = match plane {
                0 => [0, a, b],
                1 => [a, 0, b],
                _ => [a, b, 0],
            };
            out.push([center[0] + d[0], center[1] + d[1], center[2] + d[2]]);
        }
    }
    let dist = |p: &[i32; 3]| -> i64 {
        (0..3).map(|i| i64::from(p[i] - center[i]).pow(2)).sum()
    };
    out.sort_by_key(dist);
    (out, flush)
}

pub(super) fn world_primitives(game: &Game<'_>, primitives: &mut WorldPrimitives) {
    if !game.native_on(MOD_ID) || game.native_mod_screen().is_some() {
        return;
    }
    let (size, vertical, available) = with(|s| (s.wand.size, s.wand.vertical, s.wand.available));
    if size == 0 {
        return;
    }
    let Some(hit) = game.pick() else {
        return;
    };
    let Some(player) = game.native_player() else {
        return;
    };
    let (cands, flush) = candidates(hit.pos, hit.face, size, vertical, player.yaw);
    let (min_y, height) = game.native_world_height();
    let facts = game.res.facts();
    let back = [-hit.face[0], -hit.face[1], -hit.face[2]];
    let bit = 1u8 << ordinal(hit.face);
    let mut shown = 0u64;
    for p in cands {
        if p[1] < min_y || p[1] >= min_y + height {
            continue;
        }
        let here = facts.get(game.world.block(p[0], p[1], p[2]));
        if !here.replaceable {
            continue;
        }
        if flush {
            let s = [p[0] + back[0], p[1] + back[1], p[2] + back[2]];
            let support = facts.get(game.world.block(s[0], s[1], s[2]));
            if support.replaceable || support.sturdy_full & bit == 0 {
                continue;
            }
        }
        let color = if shown < available { 0xFF55_FF55 } else { 0xFFFF_AA00 };
        let min = [f64::from(p[0]) + 0.002, f64::from(p[1]) + 0.002, f64::from(p[2]) + 0.002];
        let max = [f64::from(p[0]) + 0.998, f64::from(p[1]) + 0.998, f64::from(p[2]) + 0.998];
        primitives.box_outline(min, max, color, 2.0);
        shown += 1;
    }
}

// ---------------------------------------------------------------- écran de mélange

const PANEL_W: f32 = 252.0;
const ROW_H: f32 = 20.0;
const BAR_X: f32 = 118.0;
const BAR_W: f32 = 100.0;
const BUTTON_W: f32 = 76.0;
const BUTTON_H: f32 = 18.0;

struct Layout {
    x: f32,
    y: f32,
    h: f32,
}

fn layout(screen: &NativeScreenView<'_>) -> Layout {
    let [sx, sy, sw, sh] = screen.rect;
    let h = 22.0 + ROW_H * SLOTS as f32 + 10.0 + BUTTON_H + 8.0;
    Layout {
        x: (sx + (sw - PANEL_W) / 2.0).floor(),
        y: (sy + (sh - h) / 2.0).max(sy + 2.0).floor(),
        h,
    }
}

fn row_y(l: &Layout, i: usize) -> f32 {
    l.y + 22.0 + ROW_H * i as f32
}

fn buttons_y(l: &Layout) -> f32 {
    l.y + 22.0 + ROW_H * SLOTS as f32 + 10.0
}

/// Les trois boutons : Appliquer, Orientation, Annuler.
fn button_rects(l: &Layout) -> [[f32; 4]; 3] {
    let y = buttons_y(l);
    let gap = (PANEL_W - 3.0 * BUTTON_W) / 4.0;
    std::array::from_fn(|i| [l.x + gap + (BUTTON_W + gap) * i as f32, y, BUTTON_W, BUTTON_H])
}

fn inside(r: [f32; 4], m: [f32; 2]) -> bool {
    m[0] >= r[0] && m[0] < r[0] + r[2] && m[1] >= r[1] && m[1] < r[1] + r[3]
}

fn bar_rect(l: &Layout, i: usize) -> [f32; 4] {
    [l.x + BAR_X, row_y(l, i) + 6.0, BAR_W, 6.0]
}

fn weight_at(l: &Layout, mouse_x: f32) -> i32 {
    (((mouse_x - (l.x + BAR_X)) / BAR_W) * 100.0).round().clamp(0.0, 100.0) as i32
}

fn ours(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(MOD_ID)
}

fn draw_button(game: &Game<'_>, batch: &mut HudBatch, r: [f32; 4], label: &str, hover: bool) {
    game.native_fill(batch, r, if hover { 0xFF6E_6E6E } else { 0xFF4A_4A4A });
    game.native_fill(batch, [r[0], r[1], r[2], 1.0], 0xFFA0_A0A0);
    game.native_fill(batch, [r[0], r[1] + r[3] - 1.0, r[2], 1.0], 0xFF20_2020);
    let w = game.native_text_width(label);
    game.native_text(batch, label, [r[0] + (r[2] - w) / 2.0, r[1] + 5.0], 0xFFFFFF, true);
}

fn truncate(game: &Game<'_>, text: &str, max: f32) -> String {
    if game.native_text_width(text) <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if game.native_text_width(&format!("{out}…")) > max {
            out.pop();
            break;
        }
    }
    format!("{out}…")
}

pub(super) fn screen_render_post(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !ours(game) {
        return;
    }
    let l = layout(screen);
    let mouse = screen.mouse;
    with(|s| {
        let wand = s.wand.clone();
        let Some(sc) = s.screen.as_mut() else {
            return;
        };
        if let Some(i) = sc.dragging {
            sc.weights[i] = weight_at(&l, mouse[0]);
        }
        // Fond et titre.
        game.native_fill(batch, [l.x - 1.0, l.y - 1.0, PANEL_W + 2.0, l.h + 2.0], 0xFF00_0000);
        game.native_fill(batch, [l.x, l.y, PANEL_W, l.h], 0xF02B_2B2B);
        let title = "Mélange de blocs";
        let tw = game.native_text_width(title);
        game.native_text(batch, title, [l.x + (PANEL_W - tw) / 2.0, l.y + 7.0], 0xFFFFFF, true);
        if !wand.bag {
            let msg = "Aucun sac de bâtisseur dans ton inventaire.";
            let mw = game.native_text_width(msg);
            game.native_text(batch, msg, [l.x + (PANEL_W - mw) / 2.0, row_y(&l, 4)], 0xFF5555, true);
        }
        let total: i32 = (0..SLOTS)
            .filter(|&i| wand.slots.get(i).is_some_and(|sl| sl.count > 0))
            .map(|i| sc.weights[i])
            .sum();
        for i in 0..SLOTS {
            let y = row_y(&l, i);
            let slot = wand.slots.get(i);
            let filled = slot.is_some_and(|sl| sl.count > 0);
            if let Some(sl) = slot.filter(|_| filled) {
                if !game.native_item_model(batch, &sl.item, &sl.model, [l.x + 6.0, y + 1.0], 16.0) {
                    game.native_fill(batch, [l.x + 8.0, y + 3.0, 12.0, 12.0], 0xFF80_8080);
                }
                let name = truncate(game, &sl.name, BAR_X - 30.0);
                game.native_text(batch, &name, [l.x + 26.0, y + 5.0], 0xE0E0E0, false);
            } else {
                game.native_text(batch, "(vide)", [l.x + 26.0, y + 5.0], 0x808080, false);
            }
            let r = bar_rect(&l, i);
            let w = sc.weights[i];
            game.native_fill(batch, r, 0xFF10_1010);
            let fill = r[2] * w as f32 / 100.0;
            let color = if !filled { 0xFF50_5050 } else if w == 0 { 0xFF60_6060 } else { 0xFF55_CC55 };
            game.native_fill(batch, [r[0], r[1], fill, r[3]], color);
            let knob_x = (r[0] + fill - 1.5).clamp(r[0], r[0] + r[2] - 3.0);
            game.native_fill(batch, [knob_x, r[1] - 3.0, 3.0, r[3] + 6.0], 0xFFE0_E0E0);
            let pct = if filled && total > 0 { (w * 100 + total / 2) / total } else { 0 };
            let label = if filled { format!("{w} ({pct}%)") } else { format!("{w}") };
            game.native_text(batch, &label, [r[0] + r[2] + 6.0, y + 5.0], 0xFFFF55, false);
        }
        let labels = [
            "Appliquer",
            if wand.vertical { "Verticale" } else { "Horizontale" },
            "Annuler",
        ];
        for (r, label) in button_rects(&l).into_iter().zip(labels) {
            draw_button(game, batch, r, label, inside(r, mouse));
        }
    });
}

/// Un clic sur l'écran : curseur, ou bouton. L'écran ne laisse passer aucun clic.
pub(super) fn mouse_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !ours(game) {
        return;
    }
    *canceled = true;
    if button != 0 {
        return;
    }
    let l = layout(screen);
    let mouse = screen.mouse;
    with(|s| {
        let Some(sc) = s.screen.as_mut() else {
            return;
        };
        for i in 0..SLOTS {
            let r = bar_rect(&l, i);
            if inside([r[0] - 3.0, r[1] - 5.0, r[2] + 6.0, r[3] + 10.0], mouse) {
                sc.dragging = Some(i);
                sc.weights[i] = weight_at(&l, mouse[0]);
                return;
            }
        }
        let rects = button_rects(&l);
        if inside(rects[0], mouse) {
            let weights = sc.weights.clone();
            s.pending.push(Action::Apply(weights));
        } else if inside(rects[1], mouse) {
            s.pending.push(Action::Toggle);
        } else if inside(rects[2], mouse) {
            s.pending.push(Action::Close);
        }
    });
}

pub(super) fn mouse_released(game: &Game<'_>, button: i32, canceled: &mut bool) {
    if !ours(game) {
        return;
    }
    *canceled = true;
    if button == 0 {
        with(|s| {
            if let Some(sc) = s.screen.as_mut() {
                sc.dragging = None;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_encode_like_the_java_codec() {
        // Relevé côté serveur : SetWeights(0,1,100,300,50,50,50,50,7).
        let got = encode_weights(&[0, 1, 100, 300, 50, 50, 50, 50, 7]);
        assert_eq!(got, vec![0x09, 0x00, 0x01, 0x64, 0xac, 0x02, 0x32, 0x32, 0x32, 0x32, 0x07]);
    }

    #[test]
    fn state_parses() {
        let st = parse("hb1\nwand=3\nvertical=1\nbag=1\navailable=128\nslot=61|64|minecraft:stone|minecraft:stone|Stone\nslot=50|0|||\n").unwrap();
        assert_eq!(st.size, 3);
        assert!(st.vertical && st.bag);
        assert_eq!(st.available, 128);
        assert_eq!(st.slots.len(), 2);
        assert_eq!(st.slots[0].name, "Stone");
        assert_eq!(st.slots[1].count, 0);
    }

    #[test]
    fn flat_three_by_three_on_top() {
        let (c, flush) = candidates([0, 64, 0], [0, 1, 0], 3, false, 0.0);
        assert!(flush);
        assert_eq!(c.len(), 9);
        assert_eq!(c[0], [0, 65, 0]);
        assert!(c.iter().all(|p| p[1] == 65));
    }

    #[test]
    fn horizontal_on_a_wall_sticks_out() {
        // Face est d'un mur, mode horizontal : plan horizontal centré 2 cases plus loin.
        let (c, flush) = candidates([0, 64, 0], [1, 0, 0], 3, false, 0.0);
        assert!(!flush);
        assert_eq!(c[0], [2, 64, 0]);
    }

    #[test]
    fn vertical_on_the_ground_follows_the_view() {
        // Regard vers le sud (lacet 0) : axe Z, donc un mur dans le plan X/Y.
        let (c, flush) = candidates([0, 64, 0], [0, 1, 0], 3, true, 0.0);
        assert!(!flush);
        assert!(c.iter().all(|p| p[2] == 0));
        let (c, _) = candidates([0, 64, 0], [0, 1, 0], 3, true, 90.0);
        assert!(c.iter().all(|p| p[0] == 0));
    }
}
