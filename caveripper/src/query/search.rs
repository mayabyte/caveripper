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
        s.spawn_broadcast(|_scope, _broadcast_context| {
            let mut rng = thread_rng();
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

                let seed = rng.r#gen();

                if query.matches(seed, mgr) {
                    on_found(seed);
                    if num_to_find.is_some() {
                        num_found.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
    });
}
