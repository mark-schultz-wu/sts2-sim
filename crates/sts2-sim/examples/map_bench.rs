//! A lightweight map-generation benchmark with allocation accounting.
//!
//! Run with `cargo run --release -p sts2-sim --example map_bench -- 20000`.

use std::alloc::{GlobalAlloc, Layout as AllocLayout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use clap::Parser;

use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::ascension::AscensionLevel;
use sts2_core::Rules as _;
use sts2_core::rng::GameRng;
use sts2_sim::map::ActMap;
use sts2_sim::rng::MegaRng;

struct CountingAllocator;

static COUNTING: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: AllocLayout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        // SAFETY: forwarding the allocation request unchanged to System.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: AllocLayout) {
        // SAFETY: `ptr` and `layout` came from the matching System allocation.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: AllocLayout, size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        // SAFETY: forwarding the reallocation request unchanged to System.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

type V = V0_107_1;

fn generate(seed: u64) -> ActMap<V> {
    let mut rng = MegaRng::new(seed);
    let layout = V::act_layout(sts2_content::v0_107_1::acts::Act::Overgrowth);
    ActMap::generate(&mut rng, layout, AscensionLevel::SwarmingElites).unwrap()
}

/// Time map generation, then count what it allocates.
#[derive(Parser)]
struct Cli {
    /// Maps to generate for each measure.
    #[arg(default_value_t = 20_000)]
    iterations: u32,
}

fn main() {
    let Cli { iterations } = Cli::parse();

    for seed in 0..iterations.min(25) as u64 {
        black_box(generate(seed));
    }

    let started = Instant::now();
    for seed in 0..iterations as u64 {
        black_box(generate(seed));
    }
    let elapsed = started.elapsed();

    CALLS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for seed in 0..iterations as u64 {
        black_box(generate(seed));
    }
    COUNTING.store(false, Ordering::Relaxed);

    let calls = CALLS.load(Ordering::Relaxed);
    let bytes = BYTES.load(Ordering::Relaxed);
    let nanos = elapsed.as_nanos() as f64 / f64::from(iterations);
    println!("maps: {iterations}");
    println!("elapsed: {elapsed:?}");
    println!("time/map: {nanos:.1} ns");
    println!("maps/s: {:.0}", 1_000_000_000.0 / nanos);
    println!(
        "allocation calls/map: {:.2}",
        calls as f64 / f64::from(iterations)
    );
    println!(
        "requested bytes/map: {:.1}",
        bytes as f64 / f64::from(iterations)
    );
}
