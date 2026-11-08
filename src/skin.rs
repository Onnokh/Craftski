//! WoW Forever's own interface art: pieces cut from the game's texture atlases, downloaded from
//! wago.tools once and cached as PNGs. Nothing of it ships with the app.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::blp;
use crate::cache::{self, write_atomic};
use crate::recipes::Profession;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Background,
    SlotFrame,
    ProgressBg,
    ProgressFrame,
    Dropdown,
    DropdownOpen,
    MenuBg,
    SearchLeft,
    SearchMid,
    SearchRight,
    Magnifier,
    RedLeft,
    RedMid,
    RedRight,
    RowSelected,
    TabSelected,
    Checkbox,
    Checkmark,
    /// The profession's banner, behind a tab's title.
    OverviewCard(Profession),
    /// The profession's tall card, behind the detail panel.
    BackgroundCard(Profession),
}

/// Where a piece lies: its name in the game's atlas tables, the atlas BLP's CASC file id, and
/// its `[left, top, right, bottom]` in that BLP's pixels.
pub struct Cut {
    pub name: &'static str,
    pub atlas: u32,
    pub rect: [u32; 4],
}

const fn cut(name: &'static str, atlas: u32, rect: [u32; 4]) -> Cut {
    Cut { name, atlas, rect }
}

/// The shared pieces' cuts, indexed by [`Piece::index`]. These and [`CARDS`] are from the
/// UiTextureAtlas tables of Forever build 1.60.1.70862.
const SHARED_CUTS: [Cut; Piece::SHARED.len()] = [
    cut("heavybronze-frame-background-c60", 8216846, [0, 0, 1024, 1024]),
    cut("profession-square-frame-c60-2x", 8164397, [131, 39, 211, 119]),
    cut("profession-progressbar-bg-c60-2x", 8164397, [219, 1, 967, 47]),
    cut("profession-progressbar-frame-c60-2x", 8164397, [219, 49, 967, 95]),
    cut("common-dropdown-b-button-c60-2x", 8069110, [1, 183, 195, 235]),
    cut("common-dropdown-b-button-open-c60-2x", 8069110, [197, 237, 391, 289]),
    cut("common-dropdown-bg-c60-2x", 8069110, [183, 1, 319, 137]),
    cut("common-search-border-left-c60", 8086476, [227, 1, 243, 41]),
    cut("common-search-border-middle-c60", 8086476, [1, 1, 225, 41]),
    cut("common-search-border-right-c60", 8086476, [1, 43, 17, 83]),
    cut("common-search-magnifyingglass", 3281887, [19, 85, 43, 109]),
    cut("128-redbutton-left-c60", 8063723, [391, 781, 505, 909]),
    cut("_128-redbutton-center-c60", 8063723, [0, 1, 64, 129]),
    cut("128-redbutton-right-c60", 8063723, [1, 391, 293, 519]),
    cut("auctionhouse-ui-row-select", 3046538, [827, 215, 943, 233]),
    cut("common-sidetab-selected-c60-2x", 8124215, [113, 1, 223, 121]),
    cut("checkbox-minimal-c60", 8086474, [1, 1, 31, 30]),
    cut("checkmark-minimal", 4614134, [1, 32, 31, 61]),
];

macro_rules! cards {
    ($name:literal, $atlas:literal) => {
        [
            cut(concat!("profession-overview-card-", $name, "-c60-2x"), $atlas, OVERVIEW),
            cut(concat!("profession-background-card-", $name, "-c60-2x"), $atlas, [1, 287, 721, 1255]),
        ]
    };
}

/// Each profession's overview card then background card, indexed by `Profession as usize`.
/// Each profession's 2048x2048 atlas holds its banner over its background card, but Cooking and
/// First Aid have no banner of their own and keep their card alone in a smaller atlas.
const CARDS: [[Cut; 2]; Profession::ALL.len()] = [
    cards!("alchemy", 8166382),
    cards!("blacksmithing", 8166380),
    [GENERIC_OVERVIEW, cut("profession-background-card-cooking-c60-2x", 8166378, LONE_BACKGROUND)],
    cards!("enchanting", 8166376),
    cards!("engineering", 8166374),
    [GENERIC_OVERVIEW, cut("profession-background-card-firstaid-c60-2x", 8166372, LONE_BACKGROUND)],
    cards!("leatherworking", 8166364),
    cards!("mining", 8166360),
    cards!("tailoring", 8166352),
];

const OVERVIEW: [u32; 4] = [1, 1, 1329, 285];
const GENERIC_OVERVIEW: Cut = cut("profession-overview-card-c60-2x", 8166368, OVERVIEW);
const LONE_BACKGROUND: [u32; 4] = [1, 1, 721, 969];

/// How many pieces there are, shared and per profession: the bound of [`Piece::index`].
pub const COUNT: usize = Piece::SHARED.len() + 2 * Profession::ALL.len();

impl Piece {
    /// The pieces every profession draws alike.
    pub const SHARED: [Piece; 18] = [
        Piece::Background,
        Piece::SlotFrame,
        Piece::ProgressBg,
        Piece::ProgressFrame,
        Piece::Dropdown,
        Piece::DropdownOpen,
        Piece::MenuBg,
        Piece::SearchLeft,
        Piece::SearchMid,
        Piece::SearchRight,
        Piece::Magnifier,
        Piece::RedLeft,
        Piece::RedMid,
        Piece::RedRight,
        Piece::RowSelected,
        Piece::TabSelected,
        Piece::Checkbox,
        Piece::Checkmark,
    ];

    pub fn cards(profession: Profession) -> [Piece; 2] {
        [Piece::OverviewCard(profession), Piece::BackgroundCard(profession)]
    }

    /// The shared pieces, then each profession's cards.
    pub fn all() -> impl Iterator<Item = Piece> {
        Piece::SHARED.into_iter().chain(Profession::ALL.into_iter().flat_map(Piece::cards))
    }

    /// Its place in [`Piece::all`], below [`COUNT`].
    pub fn index(self) -> usize {
        match self {
            Piece::OverviewCard(p) => Piece::SHARED.len() + 2 * p as usize,
            Piece::BackgroundCard(p) => Piece::SHARED.len() + 2 * p as usize + 1,
            shared => Piece::SHARED.iter().position(|&s| s == shared).expect("every other piece is shared"),
        }
    }

    pub fn cut(self) -> &'static Cut {
        match self {
            Piece::OverviewCard(p) => &CARDS[p as usize][0],
            Piece::BackgroundCard(p) => &CARDS[p as usize][1],
            shared => &SHARED_CUTS[shared.index()],
        }
    }

    /// Its size in points: `-2x` art draws at half its pixel size.
    pub fn size(self) -> (f32, f32) {
        let [l, t, r, b] = self.cut().rect;
        let density = if self.cut().name.ends_with("-2x") { 2.0 } else { 1.0 };
        ((r - l) as f32 / density, (b - t) as f32 / density)
    }
}

pub fn dir() -> Option<PathBuf> {
    cache::dir("ui-v1")
}

/// Named after its cut, so pieces that show the same art share one file.
pub fn path(dir: &Path, piece: Piece) -> PathBuf {
    dir.join(format!("{}.png", piece.cut().name))
}

/// The `[left, top, right, bottom]` rectangle of an RGBA image `width` pixels wide, as its own
/// RGBA image. None when the rectangle reaches past the image.
pub fn crop(width: u32, rgba: &[u8], rect: [u32; 4]) -> Option<Vec<u8>> {
    let [l, t, r, b] = rect.map(|v| v as usize);
    let width = width as usize;
    if l >= r || t >= b || r > width || b * width * 4 > rgba.len() {
        return None;
    }
    Some((t..b).flat_map(|y| &rgba[(y * width + l) * 4..(y * width + r) * 4]).copied().collect())
}

/// The nine `[left, top, right, bottom]` rectangles, row by row, that `insets` (from each edge,
/// in the same order) cut a `width`×`height` piece into: corners, edges and the middle.
pub fn nine(width: u32, height: u32, insets: [u32; 4]) -> [[u32; 4]; 9] {
    let [l, t, r, b] = insets;
    let xs = [0, l, width.saturating_sub(r), width];
    let ys = [0, t, height.saturating_sub(b), height];
    std::array::from_fn(|i| [xs[i % 3], ys[i / 3], xs[i % 3 + 1], ys[i / 3 + 1]])
}

/// Puts `pieces` on disk, calling `on_ready` from a worker thread as each one is there, cached
/// or freshly cut. An atlas downloads only when one of its pieces is missing, and is dropped
/// once its pieces are written. Blocks until every atlas has landed or failed.
pub fn fetch(pieces: &[Piece], on_ready: impl Fn(Piece, PathBuf) + Sync) -> Result<(), String> {
    let dir = dir().ok_or("LOCALAPPDATA is not set")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut atlases: Vec<u32> = pieces.iter().map(|p| p.cut().atlas).collect();
    atlases.sort_unstable();
    atlases.dedup();
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).build().into();
    let failed: Vec<String> = std::thread::scope(|scope| {
        let (dir, agent, on_ready) = (&dir, &agent, &on_ready);
        let workers: Vec<_> = atlases.into_iter().map(|atlas| scope.spawn(move || fetch_atlas(agent, dir, atlas, pieces, on_ready))).collect();
        workers.into_iter().filter_map(|w| w.join().unwrap_or_else(|_| Err("panicked".into())).err()).collect()
    });
    if failed.is_empty() { Ok(()) } else { Err(failed.join("; ")) }
}

/// Hands `fetch` the shared pieces with `first`'s cards, then each other profession's cards:
/// the one last asked for on `asked` ahead of the rest, the rest in turn. Then serves asks for
/// any that failed until `asked` closes. A profession whose cards landed is never fetched again.
pub fn fetch_in_order(first: Profession, asked: &Receiver<Profession>, mut fetch: impl FnMut(&[Piece]) -> Result<(), String>) {
    let mut done = Vec::new();
    let mut rest = Profession::ALL.into_iter().filter(|&p| p != first);
    let (mut profession, mut pieces) = (first, Piece::SHARED.into_iter().chain(Piece::cards(first)).collect::<Vec<_>>());
    loop {
        match fetch(&pieces) {
            Ok(()) => done.push(profession),
            Err(e) => eprintln!("{} art not fetched: {e}", profession.name()),
        }
        let pending = |p: &Profession| !done.contains(p);
        let Some(next) = asked.try_iter().filter(pending).last().or_else(|| rest.find(pending)).or_else(|| asked.iter().find(pending)) else { return };
        (profession, pieces) = (next, Piece::cards(next).to_vec());
    }
}

fn fetch_atlas(agent: &ureq::Agent, dir: &Path, atlas: u32, pieces: &[Piece], on_ready: &(impl Fn(Piece, PathBuf) + Sync)) -> Result<(), String> {
    let (cached, missing): (Vec<Piece>, Vec<Piece>) = pieces.iter().filter(|p| p.cut().atlas == atlas).partition(|&&p| path(dir, p).exists());
    for p in cached {
        on_ready(p, path(dir, p));
    }
    if missing.is_empty() {
        return Ok(());
    }
    let url = format!("https://wago.tools/api/casc/{atlas}?download");
    let mut response = agent.get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let bytes = response.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(|e| format!("{url}: {e}"))?;
    let (w, h, rgba) = blp::decode(&bytes).map_err(|e| format!("{url}: {e}"))?;
    drop(bytes);
    for p in missing {
        let [l, t, r, b] = p.cut().rect;
        let piece = crop(w, &rgba, p.cut().rect).ok_or_else(|| format!("{:?} reaches past its {w}x{h} atlas {atlas}", p))?;
        let png = encode_png(r - l, b - t, piece)?;
        let file = path(dir, p);
        write_atomic(&file, &png).map_err(|e| format!("{}: {e}", file.display()))?;
        on_ready(p, file);
    }
    Ok(())
}

fn encode_png(w: u32, h: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = image::RgbaImage::from_raw(w, h, rgba).ok_or("piece size and pixels disagree")?;
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_has_its_own_index_and_a_cut_named_for_it() {
        let all: Vec<Piece> = Piece::all().collect();
        assert_eq!(all.len(), COUNT);
        for (i, p) in all.into_iter().enumerate() {
            assert_eq!(p.index(), i, "{p:?}");
        }
        for p in Profession::ALL {
            let name = p.slug().replace('-', "");
            let [overview, background] = Piece::cards(p).map(|c| c.cut().name);
            assert_eq!(background, format!("profession-background-card-{name}-c60-2x"));
            let own = format!("profession-overview-card-{name}-c60-2x");
            assert!(overview == own || matches!(p, Profession::Cooking | Profession::FirstAid) && overview == GENERIC_OVERVIEW.name, "{p:?}: {overview}");
        }
    }

    /// What `fetch_in_order` hands its fetch, as the profession each batch's cards belong to,
    /// after `asks` were made; a profession in `failing` fails its first fetch.
    fn fetch_order(first: Profession, asks: &[Profession], failing: Option<Profession>) -> Vec<Profession> {
        let (ask, asked) = std::sync::mpsc::channel();
        for &p in asks {
            ask.send(p).unwrap();
        }
        drop(ask);
        let mut fetched = Vec::new();
        fetch_in_order(first, &asked, |pieces| {
            let Some(&Piece::OverviewCard(p)) = pieces.iter().find(|p| matches!(p, Piece::OverviewCard(_))) else { return Err("no cards".into()) };
            let shared = pieces.len() == Piece::SHARED.len() + 2;
            assert_eq!(shared, fetched.is_empty(), "only the first batch carries the shared pieces");
            fetched.push(p);
            if failing == Some(p) && fetched.iter().filter(|&&f| f == p).count() == 1 { Err("offline".into()) } else { Ok(()) }
        });
        fetched
    }

    #[test]
    fn fetches_the_first_professions_art_then_the_last_one_asked_for_then_the_rest_once_each() {
        use Profession::*;
        assert_eq!(fetch_order(Leatherworking, &[Mining, Tailoring, Leatherworking], None), [Leatherworking, Tailoring, Alchemy, Blacksmithing, Cooking, Enchanting, Engineering, FirstAid, Mining]);
    }

    #[test]
    fn an_ask_retries_cards_that_failed() {
        use Profession::*;
        assert_eq!(fetch_order(Cooking, &[], Some(Cooking)), [Cooking, Alchemy, Blacksmithing, Enchanting, Engineering, FirstAid, Leatherworking, Mining, Tailoring]);
        assert_eq!(fetch_order(Cooking, &[Cooking], Some(Cooking)), [Cooking, Cooking, Alchemy, Blacksmithing, Enchanting, Engineering, FirstAid, Leatherworking, Mining, Tailoring]);
    }

    #[test]
    fn crops_a_rectangle_out_of_an_image() {
        let px = |v: u8| [v, v, v, 255];
        let image: Vec<u8> = (0..12).flat_map(px).collect();
        assert_eq!(crop(4, &image, [1, 1, 3, 3]), Some([5, 6, 9, 10].into_iter().flat_map(px).collect()));
        assert_eq!(crop(4, &image, [3, 0, 5, 1]), None);
        assert_eq!(crop(4, &image, [0, 2, 1, 4]), None);
    }

    #[test]
    fn cuts_a_piece_into_corners_edges_and_middle() {
        let cells = nine(194, 52, [16, 16, 56, 16]);
        assert_eq!(cells[0], [0, 0, 16, 16]);
        assert_eq!(cells[4], [16, 16, 138, 36]);
        assert_eq!(cells[8], [138, 36, 194, 52]);
        let image = vec![0; 194 * 52 * 4];
        let area: usize = cells.iter().map(|&c| crop(194, &image, c).map_or(0, |px| px.len() / 4)).sum();
        assert_eq!(area, 194 * 52);
    }

    #[test]
    fn sizes_double_density_art_at_half_its_pixels() {
        assert_eq!(Piece::SlotFrame.size(), (40.0, 40.0));
        assert_eq!(Piece::SearchLeft.size(), (16.0, 40.0));
    }

    #[test]
    fn caches_each_piece_by_its_cut_name() {
        let dir = Path::new("cache/ui-v1");
        assert_eq!(path(dir, Piece::RedMid), Path::new("cache/ui-v1/_128-redbutton-center-c60.png"));
        assert_eq!(path(dir, Piece::BackgroundCard(Profession::Mining)), Path::new("cache/ui-v1/profession-background-card-mining-c60-2x.png"));
        assert_ne!(path(dir, Piece::BackgroundCard(Profession::Mining)), path(dir, Piece::BackgroundCard(Profession::Alchemy)));
    }
}
