use std::{time::Instant, num::NonZeroU32};
use crossbeam::channel::{Receiver, bounded, unbounded};
use indicatif::ProgressBar;
use rand::random;
use rayon::{current_num_threads, scope};
use crate::{query::Query, pikmin_math::PikminRng, assets::AssetManager};


/// Finds seeds matching the given QueryClause in parallel and sends them to the returned
/// Receiver.
/// - [deadline]: how long to search for. Passing 'None' will search until the associated channels
///   close, or until 'num' seeds have been found.
/// - [num]: how many matching seeds to find. Passing 'None' will search until the deadline is
///   reached, or until the associated channels close.
/// - [start_from]: instead of checking random seeds, checks seeds in ascending order starting
///   from the provided value.
/// - [progress]: an Indicatif ProgressBar that will be updated as the search progresses.
pub fn find_matching_layouts_parallel(
    query: &Query,
    mgr: &AssetManager,
    deadline: Option<Instant>,
    num: Option<usize>,
    start_from: Option<u32>,
    progress: Option<&ProgressBar>,
) -> Receiver<u32> {
    let (sender, results_r) = unbounded();
    let (num_s, num_r) = bounded::<()>(num.unwrap_or(0));  // 'Free-floating' way for spawned threads to communicate with each other.

    let num_threads = current_num_threads() as u32;
    scope(|s| {
        for t in 0..num_threads {
            let sender = sender.clone();
            let num_s = num_s.clone();
            let num_r = num_r.clone();
            let query = query.clone();
            let progress = progress.map(|p| p.downgrade());
            s.spawn(move |_| {
                let rng = PikminRng::new(start_from.unwrap_or_else(random));
                // Offset the rng uniquely for each thread
                if t > 0 {
                    rng.advance(NonZeroU32::new(t).unwrap());
                }

                loop {
                    if let Some(deadline_inner) = deadline && Instant::now() > deadline_inner {
                        return;
                    }

                    // num = 0 means we search either until the timeout or until the receiver closes.
                    if let Some(n) = num && num_r.len() >= n {
                        return;
                    }

                    // Advance the rng by the same amount for each thread so threads never overlap
                    // given their unique offsets.
                    rng.advance(unsafe{NonZeroU32::new_unchecked(num_threads)});
                    let seed: u32 = rng.seed();

                    if query.matches(seed, mgr) {
                        if sender.send(seed).is_err() {
                            return;
                        }
                        if num.is_some() {
                            let _ = num_s.try_send(());
                        }
                    }

                    if let Some(progress) = progress.as_ref().and_then(|p| p.upgrade()) {
                        progress.inc(1);
                    }
                }
            });
        }
    });

    results_r
}
