use crate::caveinfo::CaveUnit;

/// The sorting algorithm required by the generation algorithm for cave units.
/// This sort is unstable! Despite being essentially a bubble sort, I've
/// implemented it manually here to ensure it exactly matches the one in Pikmin 2.
pub fn sort_cave_units(mut unsorted: Vec<CaveUnit>) -> Vec<CaveUnit> {
    // This is kinda like Bubble Sort, except it compares the entire
    // remaining list to the current element rather than just the next elem.
    let mut i: isize = 0;
    while i < unsorted.len() as isize {
        let mut j = i + 1;
        while j < unsorted.len() as isize {
            if unsorted[i as usize] > unsorted[j as usize] {
                let current = unsorted.remove(i as usize);
                unsorted.push(current);
                i -= 1;
                break;
            }
            j += 1;
        }
        i += 1;
    }
    unsorted
}

/// Takes a Vec of CaveUnits and returns a vec with the same cave units, but
/// duplicated for each possible rotation.
pub fn expand_rotations(input: Vec<CaveUnit>) -> Vec<CaveUnit> {
    input.into_iter()
        .flat_map(|unit| [unit.copy_and_rotate_to(0), unit.copy_and_rotate_to(1), unit.copy_and_rotate_to(2), unit.copy_and_rotate_to(3)])
        .collect()
}
