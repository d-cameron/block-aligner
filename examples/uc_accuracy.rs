#![cfg(any(
        all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2"),
        all(target_arch = "wasm32", target_feature = "simd128")
))]

use bio::alignment::pairwise::*;
use bio::alignment::{Alignment, AlignmentOperation};
use bio::scores::blosum62;

use block_aligner::scan_block::*;
use block_aligner::scores::*;

use std::{env, cmp};
use std::fs::File;
use std::io::{BufRead, BufReader};

fn test(file_name: &str, verbose: bool, wrong: &mut [usize], wrong_avg: &mut [i64], count: &mut [usize]) {
    let reader = BufReader::new(File::open(file_name).unwrap());

    for line in reader.lines() {
        let line = line.unwrap();
        let mut last_two = line.split_ascii_whitespace().rev().take(2);
        let r = last_two.next().unwrap().to_ascii_uppercase();
        let q = last_two.next().unwrap().to_ascii_uppercase();

        // rust-bio
        let mut bio_aligner = Aligner::with_capacity(q.len(), r.len(), -10, -1, &blosum62);
        let bio_alignment = bio_aligner.global(q.as_bytes(), r.as_bytes());
        let bio_score = bio_alignment.score;
        let seq_identity = seq_id(&bio_alignment);
        let id_idx = cmp::min((seq_identity * 10.0) as usize, 9);

        let r_padded = PaddedBytes::from_bytes(r.as_bytes(), 2048, false);
        let q_padded = PaddedBytes::from_bytes(q.as_bytes(), 2048, false);
        type RunParams = GapParams<-11, -1>;

        // ours
        let block_aligner = Block::<RunParams, _, 32, 2048, false, false>::align(&q_padded, &r_padded, &BLOSUM62, 0, 2);
        let scan_score = block_aligner.res().score;

        if bio_score != scan_score {
            wrong[id_idx] += 1;
            wrong_avg[id_idx] += (bio_score - scan_score) as i64;

            if verbose {
                println!(
                    "seq id: {}, bio: {}, ours: {}\nq (len = {}): {}\nr (len = {}): {}\nbio pretty:\n{}",
                    seq_identity,
                    bio_score,
                    scan_score,
                    q.len(),
                    q,
                    r.len(),
                    r,
                    bio_alignment.pretty(q.as_bytes(), r.as_bytes())
                );
            }
        }

        count[id_idx] += 1;
    }
}

// BLAST sequence identity
fn seq_id(a: &Alignment) -> f64 {
    let mut matches = 0;

    for &op in &a.operations {
        if op == AlignmentOperation::Match {
            matches += 1;
        }
    }

    (matches as f64) / (a.operations.len() as f64)
}

fn main() {
    let arg1 = env::args().skip(1).next();
    let verbose = arg1.is_some() && arg1.unwrap() == "-v";
    let file_names = [
        "data/uc30_30_40.m8",
        "data/uc30_40_50.m8",
        "data/uc30_50_60.m8",
        "data/uc30_60_70.m8",
        "data/uc30_70_80.m8",
        "data/uc30_80_90.m8",
        "data/uc30_90_100.m8"
    ];

    let mut wrong = [0usize; 10];
    let mut wrong_avg = [0i64; 10];
    let mut count = [0usize; 10];

    for file_name in file_names.iter() {
        test(file_name, verbose, &mut wrong, &mut wrong_avg, &mut count);
    }

    println!();

    for i in 0..10 {
        println!(
            "bin: {}-{}, count: {}, wrong: {}, wrong avg: {}",
            (i as f64) / 10.0,
            ((i as f64) + 1.0) / 10.0,
            count[i],
            wrong[i],
            (wrong_avg[i] as f64) / (wrong[i] as f64)
        );
    }

    println!("\ntotal: {}, wrong: {}", count.iter().sum::<usize>(), wrong.iter().sum::<usize>());
    println!("Done!");
}