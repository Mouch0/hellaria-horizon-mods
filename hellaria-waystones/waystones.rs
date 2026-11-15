// Hellaria Waystones pour Horizon : l'écran de choix d'une waystone, comme celui du mod Java
// (nom de la waystone utilisée, recherche, liste des destinations avec leur coût, pages).
//
// Quand le joueur utilise une waystone, le serveur (hellaria-bridge 2.3+) envoie la liste des
// destinations sur `hellaria_waystones:hz_open` dès que le client a annoncé ce canal ; le choix
// repart sur `hellaria_waystones:hz_teleport` (l'identifiant de la waystone). Sans ce mod, le
// joueur Horizon garde le menu coffre du serveur.
use crate::game::Game;
use crate::game::native_hooks::NativeScreenView;
use horizon_client::hud::HudBatch;
use horizon_protocol::packets::configuration::CustomPayload;
use horizon_protocol::packets::play::{Clientbound, Serverbound};
use std::cell::RefCell;

pub(super) const MOD_ID: &str = "hellaria_waystones";
const OPEN_CHANNEL: &str = "hellaria_waystones:hz_open";
const TELEPORT_CHANNEL: &str = "hellaria_waystones:hz_teleport";
const SEARCH_MAX: usize = 32;

/// Une waystone telle que le serveur la décrit.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Stone {
    pub(super) uid: String,
    pub(super) dimension: String,
    pub(super) pos: [i32; 3],
    /// Le joueur peut payer le coût (toujours vrai pour la waystone de départ).
    pub(super) affordable: bool,
    /// Le coût tel que Waystones le décrit ("" quand c'est gratuit).
    pub(super) cost: String,
    pub(super) name: String,
}

/// Ce que le serveur a envoyé : la waystone utilisée et ses destinations, dans l'ordre du joueur.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Listing {
    pub(super) from: Stone,
    pub(super) targets: Vec<Stone>,
}

/// Ce qu'un hook qui n'a que `&Game` laisse au tick suivant.
enum Action {
    Open,
    Teleport(String),
}

#[derive(Default)]
struct Screen {
    search: String,
    page: usize,
}

#[derive(Default)]
struct State {
    registered: bool,
    listing: Listing,
    screen: Option<Screen>,
    pending: Vec<Action>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// ---------------------------------------------------------------- liste reçue

fn int(v: Option<&str>) -> i32 {
    v.and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

/// Lit la liste texte envoyée par le serveur (voir WaystoneBridge.listing).
pub(super) fn parse(text: &str) -> Option<Listing> {
    let mut lines = text.lines();
    if lines.next()? != "hw1" {
        return None;
    }
    let mut listing = Listing::default();
    let mut has_from = false;
    for line in lines {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "from" => {
                let mut p = value.splitn(6, '|');
                listing.from = Stone {
                    uid: p.next().unwrap_or("").to_owned(),
                    dimension: p.next().unwrap_or("").to_owned(),
                    pos: [int(p.next()), int(p.next()), int(p.next())],
                    affordable: true,
                    cost: String::new(),
                    name: p.next().unwrap_or("").to_owned(),
                };
                has_from = true;
            }
            "target" => {
                let mut p = value.splitn(8, '|');
                let uid = p.next().unwrap_or("").to_owned();
                let dimension = p.next().unwrap_or("").to_owned();
                let pos = [int(p.next()), int(p.next()), int(p.next())];
                let affordable = p.next().unwrap_or("1").trim() != "0";
                let cost = p.next().unwrap_or("").to_owned();
                let name = p.next().unwrap_or("").to_owned();
                if !uid.is_empty() {
                    listing.targets.push(Stone { uid, dimension, pos, affordable, cost, name });
                }
            }
            _ => {}
        }
    }
    has_from.then_some(listing)
}

/// Les destinations dont le nom contient la recherche (sans tenir compte de la casse), par index.
pub(super) fn filtered(targets: &[Stone], search: &str) -> Vec<usize> {
    let needle = search.trim().to_lowercase();
    (0..targets.len())
        .filter(|&i| needle.is_empty() || targets[i].name.to_lowercase().contains(&needle))
        .collect()
}

/// Nombre de pages pour `count` lignes à `per_page` par page (au moins une).
pub(super) fn page_count(count: usize, per_page: usize) -> usize {
    count.div_ceil(per_page.max(1)).max(1)
}

/// Où est la destination par rapport à la waystone utilisée : la distance, ou l'autre dimension.
pub(super) fn place(from: &Stone, to: &Stone) -> String {
    if from.dimension != to.dimension {
        return match to.dimension.as_str() {
            "minecraft:overworld" => "Overworld".to_owned(),
            "minecraft:the_nether" => "Nether".to_owned(),
            "minecraft:the_end" => "End".to_owned(),
            other => other.rsplit(':').next().unwrap_or(other).to_owned(),
        };
    }
    let d: f64 = (0..3)
        .map(|i| f64::from(to.pos[i] - from.pos[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    format!("{} m", d.round() as i64)
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
/// les autres hooks ont laissé.
pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_on(MOD_ID) {
        return;
    }
    if game.native_player().is_none() {
        with(|s| {
            s.registered = false;
            s.listing = Listing::default();
            s.screen = None;
            s.pending.clear();
        });
        return;
    }
    if !with(|s| s.registered) {
        // minecraft:register : les canaux que ce client écoute, séparés par un octet nul.
        let ok = send(game, "minecraft:register", OPEN_CHANNEL.as_bytes().to_vec());
        with(|s| s.registered = ok);
    }
    let pending = with(|s| std::mem::take(&mut s.pending));
    for action in pending {
        match action {
            Action::Open => {
                if game.native_mod_screen() == Some(MOD_ID) || game.native_mod_screen_open(MOD_ID) {
                    with(|s| s.screen = Some(Screen::default()));
                }
            }
            Action::Teleport(uid) => {
                send(game, TELEPORT_CHANNEL, uid.into_bytes());
                game.native_mod_screen_close(MOD_ID);
            }
        }
    }
    // L'écran a été fermé (Échap) : on oublie la recherche.
    if game.native_mod_screen() != Some(MOD_ID) {
        with(|s| s.screen = None);
    }
}

/// Un paquet du serveur : la liste des destinations, qui ouvre l'écran.
pub(super) fn client_packet(packet: &Clientbound) {
    if let Clientbound::CustomPayload(p) = packet
        && p.channel == OPEN_CHANNEL
        && let Ok(text) = std::str::from_utf8(&p.data)
        && let Some(listing) = parse(text)
    {
        with(|s| {
            s.listing = listing;
            s.pending.push(Action::Open);
        });
    }
}

// ---------------------------------------------------------------- écran

const PANEL_W: f32 = 236.0;
const ROW_H: f32 = 22.0;
const ROW_W: f32 = 220.0;
const HEADER_H: f32 = 52.0;
const FOOTER_H: f32 = 28.0;
const BUTTON_W: f32 = 70.0;
const BUTTON_H: f32 = 18.0;

struct Layout {
    x: f32,
    y: f32,
    h: f32,
    rows: usize,
}

/// Lignes de destinations qui tiennent dans une hauteur d'écran : entre 3 et 9.
pub(super) fn rows_for(height: f32) -> usize {
    (((height - HEADER_H - FOOTER_H - 16.0) / ROW_H).floor() as i64).clamp(3, 9) as usize
}

fn layout(screen: &NativeScreenView<'_>) -> Layout {
    let [sx, sy, sw, sh] = screen.rect;
    let rows = rows_for(sh);
    let h = HEADER_H + ROW_H * rows as f32 + FOOTER_H;
    Layout {
        x: (sx + (sw - PANEL_W) / 2.0).floor(),
        y: (sy + (sh - h) / 2.0).max(sy + 2.0).floor(),
        h,
        rows,
    }
}

fn search_rect(l: &Layout) -> [f32; 4] {
    [l.x + (PANEL_W - ROW_W) / 2.0, l.y + 30.0, ROW_W, 16.0]
}

fn row_rect(l: &Layout, i: usize) -> [f32; 4] {
    [
        l.x + (PANEL_W - ROW_W) / 2.0,
        l.y + HEADER_H + ROW_H * i as f32,
        ROW_W,
        ROW_H - 2.0,
    ]
}

/// Les boutons du bas : page précédente, page suivante.
fn button_rects(l: &Layout) -> [[f32; 4]; 2] {
    let y = l.y + HEADER_H + ROW_H * l.rows as f32 + 4.0;
    let side = (PANEL_W - ROW_W) / 2.0;
    [
        [l.x + side, y, BUTTON_W, BUTTON_H],
        [l.x + PANEL_W - side - BUTTON_W, y, BUTTON_W, BUTTON_H],
    ]
}

fn inside(r: [f32; 4], m: [f32; 2]) -> bool {
    m[0] >= r[0] && m[0] < r[0] + r[2] && m[1] >= r[1] && m[1] < r[1] + r[3]
}

fn ours(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(MOD_ID)
}

fn draw_button(game: &Game<'_>, batch: &mut HudBatch, r: [f32; 4], label: &str, hover: bool, enabled: bool) {
    let fill = if !enabled {
        0xFF2A_2A2A
    } else if hover {
        0xFF6E_6E6E
    } else {
        0xFF4A_4A4A
    };
    game.native_fill(batch, r, fill);
    game.native_fill(batch, [r[0], r[1], r[2], 1.0], if enabled { 0xFFA0_A0A0 } else { 0xFF50_5050 });
    game.native_fill(batch, [r[0], r[1] + r[3] - 1.0, r[2], 1.0], 0xFF20_2020);
    let w = game.native_text_width(label);
    let color = if enabled { 0xFFFFFF } else { 0x808080 };
    game.native_text(batch, label, [r[0] + (r[2] - w) / 2.0, r[1] + (r[3] - 8.0) / 2.0], color, enabled);
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
        let listing = s.listing.clone();
        let Some(sc) = s.screen.as_mut() else {
            return;
        };
        let shown = filtered(&listing.targets, &sc.search);
        let pages = page_count(shown.len(), l.rows);
        sc.page = sc.page.min(pages - 1);

        // Fond, titre et waystone utilisée.
        game.native_fill(batch, [l.x - 1.0, l.y - 1.0, PANEL_W + 2.0, l.h + 2.0], 0xFF00_0000);
        game.native_fill(batch, [l.x, l.y, PANEL_W, l.h], 0xF02B_2B2B);
        let title = "Choisir une waystone";
        let tw = game.native_text_width(title);
        game.native_text(batch, title, [l.x + (PANEL_W - tw) / 2.0, l.y + 6.0], 0xFFFFFF, true);
        let here = truncate(game, &listing.from.name, PANEL_W - 16.0);
        let hw = game.native_text_width(&here);
        game.native_text(batch, &here, [l.x + (PANEL_W - hw) / 2.0, l.y + 18.0], 0xFFFF55, true);

        // Recherche : on tape directement, Retour arrière efface.
        let sr = search_rect(&l);
        game.native_fill(batch, [sr[0] - 1.0, sr[1] - 1.0, sr[2] + 2.0, sr[3] + 2.0], 0xFFA0_A0A0);
        game.native_fill(batch, sr, 0xFF00_0000);
        if sc.search.is_empty() {
            game.native_text(batch, "Rechercher…", [sr[0] + 4.0, sr[1] + 4.0], 0x707070, false);
        } else {
            let text = truncate(game, &format!("{}_", sc.search), sr[2] - 8.0);
            game.native_text(batch, &text, [sr[0] + 4.0, sr[1] + 4.0], 0xE0E0E0, false);
        }

        // Les destinations de la page.
        if shown.is_empty() {
            let msg = if listing.targets.is_empty() {
                "Aucune autre waystone activée."
            } else {
                "Aucune waystone ne correspond."
            };
            let mw = game.native_text_width(msg);
            let r = row_rect(&l, 1);
            game.native_text(batch, msg, [l.x + (PANEL_W - mw) / 2.0, r[1] + 6.0], 0xFF5555, true);
        }
        for (row, &index) in shown.iter().skip(sc.page * l.rows).take(l.rows).enumerate() {
            let stone = &listing.targets[index];
            let r = row_rect(&l, row);
            let hover = inside(r, mouse);
            let fill = if !stone.affordable {
                0xFF2A_2A2A
            } else if hover {
                0xFF6E_6E6E
            } else {
                0xFF4A_4A4A
            };
            game.native_fill(batch, r, fill);
            game.native_fill(batch, [r[0], r[1], r[2], 1.0], if stone.affordable { 0xFFA0_A0A0 } else { 0xFF50_5050 });
            game.native_fill(batch, [r[0], r[1] + r[3] - 1.0, r[2], 1.0], 0xFF20_2020);
            // À droite : le coût (rouge s'il ne peut pas être payé), sinon la distance ou la dimension.
            let (info, info_color) = if stone.cost.is_empty() {
                (place(&listing.from, stone), 0xA0A0A0)
            } else {
                (stone.cost.clone(), if stone.affordable { 0x80FF20 } else { 0xFF5555 })
            };
            let info = truncate(game, &info, r[2] * 0.45);
            let iw = game.native_text_width(&info);
            game.native_text(batch, &info, [r[0] + r[2] - iw - 6.0, r[1] + 6.0], info_color, false);
            let name = truncate(game, &stone.name, r[2] - iw - 18.0);
            let color = if stone.affordable { 0xFFFFFF } else { 0x808080 };
            game.native_text(batch, &name, [r[0] + 6.0, r[1] + 6.0], color, stone.affordable);
        }

        // Pages.
        let [prev, next] = button_rects(&l);
        draw_button(game, batch, prev, "Précédent", inside(prev, mouse), sc.page > 0);
        draw_button(game, batch, next, "Suivant", inside(next, mouse), sc.page + 1 < pages);
        let label = format!("{} / {}", sc.page + 1, pages);
        let lw = game.native_text_width(&label);
        game.native_text(batch, &label, [l.x + (PANEL_W - lw) / 2.0, prev[1] + 5.0], 0xC0C0C0, false);
    });
}

/// Un clic sur l'écran : une destination, ou une page. L'écran ne laisse passer aucun clic.
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
        let shown = filtered(&s.listing.targets, &sc.search);
        let pages = page_count(shown.len(), l.rows);
        sc.page = sc.page.min(pages - 1);
        for (row, &index) in shown.iter().skip(sc.page * l.rows).take(l.rows).enumerate() {
            if inside(row_rect(&l, row), mouse) {
                let stone = &s.listing.targets[index];
                if stone.affordable {
                    s.pending.push(Action::Teleport(stone.uid.clone()));
                }
                return;
            }
        }
        let [prev, next] = button_rects(&l);
        if inside(prev, mouse) {
            sc.page = sc.page.saturating_sub(1);
        } else if inside(next, mouse) && sc.page + 1 < pages {
            sc.page += 1;
        }
    });
}

pub(super) fn mouse_released(game: &Game<'_>, canceled: &mut bool) {
    if ours(game) {
        *canceled = true;
    }
}

/// La molette tourne les pages.
pub(super) fn mouse_scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !ours(game) {
        return;
    }
    *canceled = true;
    let rows = layout(screen).rows;
    with(|s| {
        let Some(sc) = s.screen.as_mut() else {
            return;
        };
        let pages = page_count(filtered(&s.listing.targets, &sc.search).len(), rows);
        if scroll[1] < 0.0 {
            sc.page = (sc.page + 1).min(pages - 1);
        } else if scroll[1] > 0.0 {
            sc.page = sc.page.saturating_sub(1);
        }
    });
}

/// Une touche : Retour arrière efface la recherche, les pages se tournent aussi au clavier.
/// Échap est laissé au jeu, qui ferme l'écran ; aucune autre touche n'atteint le jeu.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if !ours(game) || key == "key.keyboard.escape" {
        return;
    }
    *canceled = true;
    let rows = layout(screen).rows;
    with(|s| {
        let Some(sc) = s.screen.as_mut() else {
            return;
        };
        let pages = page_count(filtered(&s.listing.targets, &sc.search).len(), rows);
        match key {
            "key.keyboard.backspace" => {
                sc.search.pop();
                sc.page = 0;
            }
            "key.keyboard.page.down" => sc.page = (sc.page + 1).min(pages - 1),
            "key.keyboard.page.up" => sc.page = sc.page.saturating_sub(1),
            _ => {}
        }
    });
}

/// Un caractère tapé : il s'ajoute à la recherche.
pub(super) fn char_typed(game: &Game<'_>, c: char, canceled: &mut bool) {
    if !ours(game) {
        return;
    }
    *canceled = true;
    if c.is_control() {
        return;
    }
    with(|s| {
        if let Some(sc) = s.screen.as_mut()
            && sc.search.chars().count() < SEARCH_MAX
        {
            sc.search.push(c);
            sc.page = 0;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "hw1\nfrom=11111111-1111-1111-1111-111111111111|minecraft:overworld|10|64|-20|Spawn\ntarget=22222222-2222-2222-2222-222222222222|minecraft:overworld|13|64|-16|1||Ferme a fer\ntarget=33333333-3333-3333-3333-333333333333|minecraft:the_nether|0|70|0|0|3 niveaux|Hub du Nether\n";

    #[test]
    fn listing_parses() {
        let l = parse(SAMPLE).unwrap();
        assert_eq!(l.from.name, "Spawn");
        assert_eq!(l.from.pos, [10, 64, -20]);
        assert_eq!(l.targets.len(), 2);
        assert_eq!(l.targets[0].name, "Ferme a fer");
        assert!(l.targets[0].affordable && l.targets[0].cost.is_empty());
        assert!(!l.targets[1].affordable);
        assert_eq!(l.targets[1].cost, "3 niveaux");
        assert_eq!(l.targets[1].uid, "33333333-3333-3333-3333-333333333333");
    }

    #[test]
    fn other_text_is_refused() {
        assert!(parse("hb1\nwand=3\n").is_none());
        assert!(parse("hw1\n").is_none());
        // Une destination sans identifiant est ignorée.
        let l = parse("hw1\nfrom=a|minecraft:overworld|0|0|0|X\ntarget=|d|0|0|0|1||Y\n").unwrap();
        assert!(l.targets.is_empty());
    }

    #[test]
    fn search_ignores_case() {
        let l = parse(SAMPLE).unwrap();
        assert_eq!(filtered(&l.targets, ""), vec![0, 1]);
        assert_eq!(filtered(&l.targets, " NETHER "), vec![1]);
        assert!(filtered(&l.targets, "zzz").is_empty());
    }

    #[test]
    fn place_is_a_distance_or_a_dimension() {
        let l = parse(SAMPLE).unwrap();
        assert_eq!(place(&l.from, &l.targets[0]), "5 m");
        assert_eq!(place(&l.from, &l.targets[1]), "Nether");
    }

    #[test]
    fn pages_and_rows() {
        assert_eq!(page_count(0, 6), 1);
        assert_eq!(page_count(6, 6), 1);
        assert_eq!(page_count(7, 6), 2);
        assert_eq!(rows_for(100.0), 3);
        assert_eq!(rows_for(240.0), 6);
        assert_eq!(rows_for(2000.0), 9);
    }
}
