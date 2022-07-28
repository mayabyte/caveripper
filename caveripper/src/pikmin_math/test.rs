use std::fs::read_to_string;
use super::{PikminRng, sqrt};

const TEST_SEED: u32 = 0x12345678u32;

#[test]
fn test_raw_rng() {
    let expected = read_to_string("../reference/rng_1mil.txt").unwrap()
        .lines()
        .map(|line| line.parse::<u32>().unwrap())
        .collect::<Vec<u32>>();
    let rng = PikminRng::new(TEST_SEED);
    let actual = (0..1_000_000).map(|_| rng.rand_raw()).collect::<Vec<u32>>();
    assert_eq!(expected, actual);
}

#[test]
fn test_rand_int() {
    let expected = read_to_string("../reference/randInt_65k.txt").unwrap()
        .lines()
        .map(|line| line.parse::<u32>().unwrap())
        .collect::<Vec<u32>>();
    let rng = PikminRng::new(TEST_SEED);
    let actual = (0..65536).map(|i| rng.rand_int(i)).collect::<Vec<u32>>();

    assert_eq!(expected, actual);
}

#[test]
fn test_rand_f32() {
    let expected = read_to_string("../reference/randFloat_100k.txt").unwrap()
        .lines()
        .map(|line| line.parse::<f32>().unwrap())
        .collect::<Vec<f32>>();
    let rng = PikminRng::new(TEST_SEED);
    let actual = (0..100_000).map(|_| rng.rand_f32()).collect::<Vec<f32>>();
    assert_eq!(expected, actual);
}

#[test]
fn test_rand_backs() {
    let expected: Vec<Vec<u32>> = read_to_string("../reference/randBacks_2k.txt").unwrap()
        .lines()
        .map(|line| line.split_terminator(',').map(|num| num.parse::<u32>().unwrap()).collect())
        .collect();
    let rng = PikminRng::new(TEST_SEED);
    let actual: Vec<Vec<u32>> = (0..2000).map(|i| {
        let mut list = (0..i).collect();
        rng.rand_backs(&mut list);
        list
    })
    .collect();
    assert_eq!(expected, actual);
}

#[test]
fn test_rand_swaps() {
    let expected: Vec<Vec<u32>> = read_to_string("../reference/randSwaps_2k.txt").unwrap()
        .lines()
        .map(|line| line.split_terminator(',').map(|num| num.parse::<u32>().unwrap()).collect())
        .collect();
    let rng = PikminRng::new(TEST_SEED);
    let actual: Vec<Vec<u32>> = (0..2000).map(|i| {
        let mut list = (0..i).collect();
        rng.rand_swaps(&mut list);
        list
    })
    .collect();
    assert_eq!(expected, actual);
}

// #[test]
// fn test_rand_index_weight() {
//     let expected: Vec<isize> = read_to_string("../reference/randIndexWeight_10k.txt").unwrap()
//         .lines()
//         .take(10000)
//         .map(|line| line.parse::<isize>().unwrap())
//         .collect();
//     let rng = PikminRng::new(TEST_SEED);
//     let actual: Vec<isize> = (0..10_000).map(|i| {
//         let list: Vec<u16> = (0..i as u16).collect();
//         rng.rand_index_weight(list.as_slice()).map(|x| x as isize).unwrap_or(-1)
//     })
//     .collect();

//     assert_eq!(expected, actual);
// }

#[test]
fn test_sqrt() {
    let expected: Vec<f32> = read_to_string("../reference/sqrt_5k_point3.txt").unwrap()
        .lines()
        .map(|line| line.parse::<f32>().unwrap())
        .collect();
    let mut actual: Vec<f32> = Vec::new();
    let mut f: f32 = 0.0;
    while f < 5000.0 {
        actual.push(sqrt(f));
        f += 0.3f32;
    }

    for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
        assert_eq!(e, a, "{}", i);
    }
}
