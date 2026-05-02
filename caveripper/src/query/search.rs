use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use rand::{thread_rng, Rng};
use rayon::scope;

use super::Query;
use crate::assets::AssetManager;

/// Finds seeds matching the given QueryClause in parallel and sends them to the returned
/// Receiver.
/// - `deadline`: how long to search for. Passing 'None' will search until the associated channels
///   close, or until 'num' seeds have been found.
/// - `num_to_find`: how many matching seeds to find. Passing 'None' will search until the deadline is
///   reached, or until the associated channels close.
/// - `on_tick`: a callback to run before checking every seed.
/// - `on_found`: a callback that's run for each found seed. This is where you need to extract the
///   matching seeds according to your needs.
pub fn find_matching_layouts_parallel<T: Fn() + Send + Sync, F: Fn(u32) + Send + Sync>(
    query: &(impl Query + Send + Sync),
    mgr: &(impl AssetManager + Send + Sync),
    deadline: Option<Instant>,
    num_to_find: Option<usize>,
    on_tick: Option<T>,
    on_found: F,
) {
    let num_found = AtomicUsize::new(0);

    scope(|s| {
        s.spawn_broadcast(|_scope, broadcast_context| {
            // divide by 2 before dividing by num threads because the most significant bit is not used in cave generation
            let seed_space_size = u32::MAX / 2 / (rayon::current_num_threads() as u32);
            let min_seed: u32 = seed_space_size * (broadcast_context.index() as u32);
            let max_seed: u32 = min_seed + seed_space_size - 1;
            let mut rng = thread_rng();
            // end at a random seed between min and max to avoid searching the same seeds every time when not doing an exhaustive search
            let final_seed: u32 = min_seed + rng.gen_range(0..seed_space_size);
            let mut curr_seed = final_seed;
            loop {
                if let Some(deadline_inner) = deadline
                    && Instant::now() > deadline_inner
                {
                    return;
                }

                // num = 0 means we search either until the timeout or until the receiver closes.
                if let Some(n) = num_to_find
                    && num_found.load(Ordering::Relaxed) >= n
                {
                    return;
                }

                if let Some(f) = on_tick.as_ref() {
                    f();
                }

                curr_seed = curr_seed + 1;
                if curr_seed > max_seed {
                    curr_seed = min_seed;
                }

                if query.matches(curr_seed, mgr) {
                    on_found(curr_seed);
                    if num_to_find.is_some() {
                        num_found.fetch_add(1, Ordering::Relaxed);
                    }
                }
                
                if curr_seed == final_seed { // return if we've searched the whole space
                    return;
                }
            }
        });
    });
}
