#![cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "wasm32"))]

use rand::prelude::*;

use bio::alignment::pairwise::*;
use bio::scores::blosum62;

use block_aligner::scan_block::*;
use block_aligner::scores::*;
use block_aligner::simulate::*;

use std::{env, str, cmp};

fn test(iter: usize, len: usize, k: usize, slow: bool, insert_len: Option<usize>, verbose: bool) -> (usize, f64, i32, i32) {
    let mut wrong = 0usize;
    let mut wrong_avg = 0i64;
    let mut wrong_min = i32::MAX;
    let mut wrong_max = i32::MIN;
    let mut rng = StdRng::seed_from_u64(1234);

    for _i in 0..iter {
        let r = rand_str(len, &AMINO_ACIDS, &mut rng);
        let q = match insert_len {
            Some(len) => rand_mutate_insert(&r, k, &AMINO_ACIDS, len, &mut rng),
            None => rand_mutate(&r, k, &AMINO_ACIDS, &mut rng)
        };

        // rust-bio
        let mut bio_aligner = Aligner::with_capacity(q.len(), r.len(), -10, -1, &blosum62);
        let bio_score = bio_aligner.global(&q, &r).score;

        let r_padded = PaddedBytes::from_bytes(&r, 256, false);
        let q_padded = PaddedBytes::from_bytes(&q, 256, false);
        type RunParams = GapParams<-11, -1>;

        // ours
        let scan_score = if slow {
            slow_align(&q, &r)
        } else {
            let block_aligner = Block::<RunParams, _, 16, 256, false, false>::align(&q_padded, &r_padded, &BLOSUM62, 0, 6);
            block_aligner.res().score
        };

        if bio_score != scan_score {
            wrong += 1;
            let score_diff = bio_score - scan_score;
            wrong_avg += score_diff as i64;
            wrong_min = cmp::min(wrong_min, score_diff);
            wrong_max = cmp::max(wrong_max, score_diff);

            if verbose {
                println!(
                    "bio: {}, ours: {}\nq: {}\nr: {}",
                    bio_score,
                    scan_score,
                    str::from_utf8(&q).unwrap(),
                    str::from_utf8(&r).unwrap()
                );
            }
        }
    }

    (wrong, (wrong_avg as f64) / (wrong as f64), wrong_min, wrong_max)
}

fn main() {
    let arg1 = env::args().skip(1).next();
    let slow = false;
    let insert_len_fn = |x: usize| Some(x / 10);
    //let insert_len = |x: usize| None;
    let verbose = arg1.is_some() && arg1.unwrap() == "-v";
    let iter = 100;
    let lens = [100, 1000];
    let rcp_ks = [20.0, 10.0, 5.0];
    /*let lens = [10, 20, 100];
    let rcp_ks = [10.0, 5.0];*/

    let mut total_wrong = 0usize;
    let mut total = 0usize;

    for &len in &lens {
        for &rcp_k in &rcp_ks {
            let (wrong, wrong_avg, wrong_min, wrong_max) = test(iter, len, ((len as f64) / rcp_k) as usize, slow, insert_len_fn(len), verbose);
            println!(
                "\nlen: {}, k: {}, iter: {}, wrong: {}, wrong avg: {}, wrong min: {}, wrong max: {}\n",
                len,
                ((len as f64) / rcp_k) as usize,
                iter,
                wrong,
                wrong_avg,
                wrong_min,
                wrong_max
            );
            total_wrong += wrong;
            total += iter;
        }
    }

    println!("\ntotal: {}, wrong: {}", total, total_wrong);
    println!("Done!");
}

#[allow(non_snake_case)]
fn slow_align(q: &[u8], r: &[u8]) -> i32 {
    let mut block_width = 16usize;
    let mut block_height = 16usize;
    let block_grow = 16usize;
    let max_size = 256usize;
    let i_step = 4usize;
    let j_step = 4usize;
    let mut y_drop = 3i32;
    let y_drop_grow = 2i32;

    let mut D = vec![i32::MIN; (q.len() + 1 + max_size) * (r.len() + 1 + max_size)];
    let mut R = vec![i32::MIN; (q.len() + 1 + max_size) * (r.len() + 1 + max_size)];
    let mut C = vec![i32::MIN; (q.len() + 1 + max_size) * (r.len() + 1 + max_size)];
    D[0 + 0 * (q.len() + 1)] = 0;
    let mut i = 0usize;
    let mut j = 0usize;
    let mut dir = 0;
    let mut best_max = 0;

    //println!("start");

    loop {
        let max = match dir {
            0 => { // right
                calc_block(q, r, &mut D, &mut R, &mut C, i, j, block_width, block_height, -11, -1)
            },
            _ => { // down
                calc_block(q, r, &mut D, &mut R, &mut C, i, j, block_width, block_height, -11, -1)
            }
        };

        if i + block_height > q.len() && j + block_width > r.len() {
            break;
        }

        let right_max = block_max(&D, q.len() + 1, i, j + block_width - 1, 1, block_height);
        let down_max = block_max(&D, q.len() + 1, i + block_height - 1, j, block_width, 1);
        best_max = cmp::max(best_max, max);

        if block_width < max_size && cmp::max(right_max, down_max) < best_max - y_drop {
            block_width += block_grow;
            block_height += block_grow;
            y_drop += y_drop_grow;
            //println!("i: {}, j: {}, w: {}", i, j, block_width);
            continue;
        }

        if j + block_width > r.len() {
            i += i_step;
            dir = 1;
        } else if i + block_height > q.len() {
            j += j_step;
            dir = 0;
        } else {
            if down_max > right_max {
                i += i_step;
                dir = 1;
            } else if right_max > down_max {
                j += j_step;
                dir = 0;
            } else {
                j += j_step;
                dir = 0;
            }
        }
    }

    D[q.len() + r.len() * (q.len() + 1)]
}

#[allow(non_snake_case)]
fn block_max(D: &[i32], col_len: usize, start_i: usize, start_j: usize, block_width: usize, block_height: usize) -> i32 {
    let mut max = i32::MIN;
    for i in start_i..start_i + block_height {
        for j in start_j..start_j + block_width {
            max = cmp::max(max, D[i + j * col_len]);
        }
    }
    max
}

#[allow(non_snake_case)]
fn calc_block(q: &[u8], r: &[u8], D: &mut [i32], R: &mut [i32], C: &mut [i32], start_i: usize, start_j: usize, block_width: usize, block_height: usize, gap_open: i32, gap_extend: i32) -> i32 {
    let idx = |i: usize, j: usize| { i + j * (q.len() + 1) };
    let mut max = i32::MIN;

    for i in start_i..start_i + block_height {
        for j in start_j..start_j + block_width {
            if D[idx(i, j)] != i32::MIN {
                continue;
            }

            R[idx(i, j)] = if i == 0 { i32::MIN } else { cmp::max(
                R[idx(i - 1, j)].saturating_add(gap_extend),
                D[idx(i - 1, j)].saturating_add(gap_open)
            ) };
            C[idx(i, j)] = if j == 0 { i32::MIN } else { cmp::max(
                C[idx(i, j - 1)].saturating_add(gap_extend),
                D[idx(i, j - 1)].saturating_add(gap_open)
            ) };
            D[idx(i, j)] = cmp::max(
                if i == 0 || j == 0 || i > q.len() || j > r.len() { i32::MIN } else {
                    D[idx(i - 1, j - 1)].saturating_add(blosum62(q[i - 1], r[j - 1]))
                },
                cmp::max(R[idx(i, j)], C[idx(i, j)])
            );
            max = cmp::max(max, D[idx(i, j)]);
        }
    }

    max
}
