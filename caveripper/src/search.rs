use std::time::Instant;
use crossbeam::channel::{Receiver, bounded, unbounded};
use indicatif::ProgressBar;
use rand::random;
use rayon::{spawn, current_num_threads};
use crate::query::QueryClause;


/// Finds seeds matching the given QueryClause in parallel and sends them to the returned
/// Receiver.
/// - deadline: how long to search for. Passing 'None' will search until the associated channels
///   close, or until 'num' seeds have been found.
/// - num: how many matching seeds to find. Passing 'None' will search until the deadline is
///   reached, or until the associated channels close.
/// - seed_source: A channel receiver that supplies the seeds to check. Random seeds are
///   searched if 'None' is passed.
/// - progress: an Indicatif ProgressBar that will be updated as the search progresses.
pub fn find_matching_layouts_parallel(
    query: &QueryClause, 
    deadline: Option<Instant>, 
    num: Option<usize>, 
    seed_source: Option<Receiver<u32>>, 
    progress: Option<&ProgressBar>
) -> Receiver<u32> {
    let (sender, results_r) = unbounded();
    let (num_s, num_r) = bounded::<()>(num.unwrap_or(0));  // 'Free-floating' way for spawned threads to communicate with each other.
    
    for _ in 0..current_num_threads() {
        let sender = sender.clone();
        let num_s = num_s.clone();
        let num_r = num_r.clone();
        let query = query.clone();
        let seed_source = seed_source.clone();
        let progress = progress.map(|p| p.downgrade());
        spawn(move || {
            loop {
                if let Some(deadline_inner) = deadline && Instant::now() > deadline_inner {
                    return;
                }

                // num = 0 means we search either until the timeout or until the receiver closes.
                if let Some(n) = num && num_r.len() >= n {
                    return;
                }

                // Attempt to retrieve a seed from the seed source, which will most likely be
                // another search function running in parallel. This allows us to search only
                // a subset of seeds that were found by some previous condition.
                let seed: u32 = if let Some(seed_source) = seed_source.as_ref() {
                    match seed_source.recv() {
                        Ok(seed) => seed,
                        Err(_) => return, // If the source is done sending seeds, then so are we.
                    }
                }
                else {
                    random()
                };

                if query.matches(seed) {
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

    results_r
}
