use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

const DECODE_WORKERS: usize = 4;

use egui::{ColorImage, TextureHandle, TextureOptions};

use crate::session;

pub const DEFAULT_THUMB_W: u32 = 96;
pub const DEFAULT_THUMB_H: u32 = 96;
pub const DEFAULT_CAPACITY: usize = 200;

/// LRU state without any texture upload — factored out so the eviction logic
/// is testable without an egui context.
pub struct Lru<K, V>
where
    K: Copy + Eq + std::hash::Hash,
{
    cap: usize,
    order: VecDeque<K>,
    map: HashMap<K, V>,
}

impl<K, V> Lru<K, V>
where
    K: Copy + Eq + std::hash::Hash,
{
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            order: VecDeque::new(),
            map: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }

    pub fn get(&mut self, k: &K) -> Option<&V> {
        if !self.map.contains_key(k) {
            return None;
        }
        self.touch(*k);
        self.map.get(k)
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<K> {
        use std::collections::hash_map::Entry;
        match self.map.entry(k) {
            Entry::Occupied(mut e) => {
                e.insert(v);
                self.touch(k);
                None
            }
            Entry::Vacant(e) => {
                e.insert(v);
                self.order.push_back(k);
                if self.map.len() > self.cap {
                    if let Some(evicted) = self.order.pop_front() {
                        self.map.remove(&evicted);
                        return Some(evicted);
                    }
                }
                None
            }
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.order.clear();
        self.map.clear();
    }

    fn touch(&mut self, k: K) {
        if let Some(pos) = self.order.iter().position(|x| *x == k) {
            self.order.remove(pos);
        }
        self.order.push_back(k);
    }
}

/// Background thumbnail decoder + memory-only LRU of `TextureHandle`s.
pub struct ThumbCache {
    lru: Lru<usize, TextureHandle>,
    pending: HashSet<usize>,
    req_tx: Sender<Request>,
    resp_rx: Receiver<Response>,
}

enum Request {
    Decode(usize),
    Shutdown,
}

struct Response {
    index: usize,
    image: ColorImage,
}

impl ThumbCache {
    pub fn new(
        capacity: usize,
        frames_dir: PathBuf,
        thumb_w: u32,
        thumb_h: u32,
        ctx: egui::Context,
    ) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<Request>();
        let (resp_tx, resp_rx) = mpsc::channel::<Response>();
        let shared_rx = Arc::new(Mutex::new(req_rx));
        for _ in 0..DECODE_WORKERS {
            let rx = Arc::clone(&shared_rx);
            let tx = resp_tx.clone();
            let worker_ctx = ctx.clone();
            let dir = frames_dir.clone();
            thread::spawn(move || decode_worker(rx, tx, worker_ctx, dir, thumb_w, thumb_h));
        }
        drop(resp_tx);
        Self {
            lru: Lru::new(capacity),
            pending: HashSet::new(),
            req_tx,
            resp_rx,
        }
    }

    /// Drain any decoded thumbnails and upload them as textures.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(resp) = self.resp_rx.try_recv() {
            self.pending.remove(&resp.index);
            let name = format!("frammpeg-thumb-{}", resp.index);
            let tex = ctx.load_texture(name, resp.image, TextureOptions::LINEAR);
            self.lru.insert(resp.index, tex);
        }
    }

    /// Return a texture if we already have one; caller should also `request()`
    /// misses so the decoder queue fills up.
    pub fn get(&mut self, index: usize) -> Option<TextureHandle> {
        self.lru.get(&index).cloned()
    }

    /// Enqueue a decode if we don't already have this thumbnail cached or
    /// pending.
    pub fn request(&mut self, index: usize) {
        if self.lru.contains(&index) || self.pending.contains(&index) {
            return;
        }
        self.pending.insert(index);
        // Best-effort: if the worker has died we don't retry.
        let _ = self.req_tx.send(Request::Decode(index));
    }
}

impl Drop for ThumbCache {
    fn drop(&mut self) {
        for _ in 0..DECODE_WORKERS {
            let _ = self.req_tx.send(Request::Shutdown);
        }
    }
}

fn decode_worker(
    req_rx: Arc<Mutex<Receiver<Request>>>,
    resp_tx: Sender<Response>,
    ctx: egui::Context,
    frames_dir: PathBuf,
    thumb_w: u32,
    thumb_h: u32,
) {
    loop {
        let msg = {
            let rx = req_rx.lock().unwrap();
            rx.recv()
        };
        let Ok(msg) = msg else { break };
        match msg {
            Request::Shutdown => break,
            Request::Decode(idx) => {
                let path = session::frame_path(&frames_dir, idx);
                let Ok(img) = image::open(&path) else {
                    continue;
                };
                let thumb = img
                    .resize_to_fill(thumb_w, thumb_h, image::imageops::FilterType::Lanczos3)
                    .to_rgba8();
                let (w, h) = (thumb.width() as usize, thumb.height() as usize);
                let color = ColorImage::from_rgba_unmultiplied([w, h], thumb.as_raw());
                if resp_tx
                    .send(Response {
                        index: idx,
                        image: color,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lru_evicts_oldest_when_capacity_exceeded() {
        let mut lru: Lru<usize, u32> = Lru::new(3);
        assert!(lru.insert(1, 10).is_none());
        assert!(lru.insert(2, 20).is_none());
        assert!(lru.insert(3, 30).is_none());
        assert_eq!(lru.insert(4, 40), Some(1));
        assert!(!lru.contains(&1));
        assert!(lru.contains(&2));
        assert!(lru.contains(&3));
        assert!(lru.contains(&4));
    }

    #[test]
    fn lru_get_refreshes_recency() {
        let mut lru: Lru<usize, u32> = Lru::new(3);
        lru.insert(1, 10);
        lru.insert(2, 20);
        lru.insert(3, 30);
        // Touching 1 should push it to MRU; 2 becomes LRU.
        assert_eq!(lru.get(&1), Some(&10));
        assert_eq!(lru.insert(4, 40), Some(2));
        assert!(lru.contains(&1));
        assert!(!lru.contains(&2));
    }

    #[test]
    fn lru_reinsert_replaces_and_touches() {
        let mut lru: Lru<usize, u32> = Lru::new(2);
        lru.insert(1, 10);
        lru.insert(2, 20);
        assert!(lru.insert(1, 99).is_none());
        assert_eq!(lru.get(&1), Some(&99));
        // 2 is LRU, so a new insert should evict 2, not 1.
        assert_eq!(lru.insert(3, 30), Some(2));
    }

    #[test]
    fn lru_clear_resets_state() {
        let mut lru: Lru<usize, u32> = Lru::new(2);
        lru.insert(1, 10);
        lru.insert(2, 20);
        lru.clear();
        assert_eq!(lru.len(), 0);
        assert!(!lru.contains(&1));
    }

    #[test]
    fn lru_capacity_of_zero_becomes_one() {
        let mut lru: Lru<usize, u32> = Lru::new(0);
        lru.insert(1, 10);
        assert!(lru.contains(&1));
    }

    /// Decode a fixture PNG through the same `resize_to_fill` path the worker
    /// thread uses, and confirm the output is exactly the requested square
    /// (fill/cover crop).
    #[test]
    fn thumbnail_decode_fill_covers() {
        let tmp = std::env::temp_dir().join(format!(
            "frammpeg-thumb-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        // 16:9 fixture. resize_to_fill(96, 96) should crop to exactly 96x96.
        let src = image::RgbaImage::from_pixel(160, 90, image::Rgba([120, 200, 90, 255]));
        let path = tmp.join("frame-0001.png");
        src.save(&path).unwrap();

        let img = image::open(&path).unwrap();
        let thumb = img
            .resize_to_fill(96, 96, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        assert_eq!(thumb.width(), 96);
        assert_eq!(thumb.height(), 96);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
