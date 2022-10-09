use std::time::Instant;
use crossbeam::channel::{Receiver, bounded, unbounded};
use indicatif::ProgressBar;
use rand::{rngs::SmallRng, SeedableRng, Rng};
use rayon::{spawn, current_num_threads};
use crate::query::Query;


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
    deadline: Option<Instant>,
    num: Option<usize>,
    start_from: Option<u32>,
    progress: Option<&ProgressBar>,
) -> Receiver<u32> {
    let (sender, results_r) = unbounded();
    let (num_s, num_r) = bounded::<()>(num.unwrap_or(0));  // 'Free-floating' way for spawned threads to communicate with each other.

    for t in 0..current_num_threads() as u32 {
        let sender = sender.clone();
        let num_s = num_s.clone();
        let num_r = num_r.clone();
        let query = query.clone();
        let progress = progress.map(|p| p.downgrade());
        spawn(move || {
            let mut iter = 0;
            let mut rng = SmallRng::from_entropy();
            loop {
                if let Some(deadline_inner) = deadline && Instant::now() > deadline_inner {
                    return;
                }

                // num = 0 means we search either until the timeout or until the receiver closes.
                if let Some(n) = num && num_r.len() >= n {
                    return;
                }

                let seed: u32 = match start_from {
                    Some(s) => {
                        iter += 1;
                        s + t * iter
                    },
                    None => rng.gen()
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
