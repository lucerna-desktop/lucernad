use std::time::{Duration, Instant};

use lucernad_spectrum::analysis::LOW_FFT_SIZE as FFT_SIZE;
use lucernad_spectrum::mix::Rings;

const RATE: u32 = 48_000;

#[test]
fn rings_sum_fresh_nodes() {
    let t0 = Instant::now();
    let mut rings = Rings::new();
    rings.push(1, RATE, &[0.25; FFT_SIZE], t0);
    rings.push(2, RATE, &[0.5; FFT_SIZE], t0);
    assert_eq!(rings.window(t0), vec![0.75; FFT_SIZE]);
}

#[test]
fn window_keeps_the_last_samples() {
    let t0 = Instant::now();
    let mut rings = Rings::new();
    let total = FFT_SIZE + 904;
    let ramp: Vec<f32> = (0..total).map(|i| i as f32).collect();
    rings.push(1, RATE, &ramp, t0);
    let window = rings.window(t0);
    assert_eq!(window.len(), FFT_SIZE);
    assert_eq!(window[0], 904.0);
    assert_eq!(window[FFT_SIZE - 1], (total - 1) as f32);

    let mut rings = Rings::new();
    rings.push(1, RATE, &[1.0; 10], t0);
    let window = rings.window(t0);
    assert!(window[..FFT_SIZE - 10].iter().all(|&s| s == 0.0));
    assert!(window[FFT_SIZE - 10..].iter().all(|&s| s == 1.0));
}

#[test]
fn stale_ring_counts_silence() {
    let t0 = Instant::now();
    let later = t0 + Duration::from_millis(150);
    let mut rings = Rings::new();
    rings.push(1, RATE, &[0.25; FFT_SIZE], t0);
    assert_eq!(rings.window(later), vec![0.0; FFT_SIZE]);

    rings.push(2, RATE, &[0.5; FFT_SIZE], later);
    assert_eq!(rings.window(later), vec![0.5; FFT_SIZE]);
}

#[test]
fn retain_drops_nodes() {
    let t0 = Instant::now();
    let mut rings = Rings::new();
    rings.push(1, RATE, &[0.25; FFT_SIZE], t0);
    rings.push(2, RATE, &[0.5; FFT_SIZE], t0);
    rings.retain(&[2]);
    assert_eq!(rings.window(t0), vec![0.5; FFT_SIZE]);
    rings.clear();
    assert_eq!(rings.window(t0), vec![0.0; FFT_SIZE]);
}

#[test]
fn one_rate_per_window() {
    let t0 = Instant::now();
    let later = t0 + Duration::from_millis(150);
    let mut rings = Rings::new();
    assert_eq!(rings.rate(t0), None);
    rings.push(2, 44_100, &[0.5; FFT_SIZE], t0);
    rings.push(1, RATE, &[0.25; FFT_SIZE], t0);
    // Dois nós com taxas diferentes: vale a do menor serial, e o outro fica
    // fora da soma (somar taxas diferentes mistura frequências).
    assert_eq!(rings.rate(t0), Some(RATE));
    assert_eq!(rings.window(t0), vec![0.25; FFT_SIZE]);
    rings.push(2, 44_100, &[0.5; 10], t0);
    assert_eq!(rings.rate(t0), Some(RATE));

    // O nó da taxa escolhida para: vale a do outro.
    rings.push(2, 44_100, &[0.5; FFT_SIZE], later);
    assert_eq!(rings.rate(later), Some(44_100));
    assert_eq!(rings.window(later), vec![0.5; FFT_SIZE]);
    assert_eq!(rings.rate(later + Duration::from_millis(150)), None);
}

#[test]
fn a_new_rate_restarts_the_ring() {
    let t0 = Instant::now();
    let mut rings = Rings::new();
    rings.push(1, 44_100, &[0.5; FFT_SIZE], t0);
    rings.push(1, RATE, &[0.25; 10], t0);
    let window = rings.window(t0);
    assert_eq!(rings.rate(t0), Some(RATE));
    assert!(window[..FFT_SIZE - 10].iter().all(|&s| s == 0.0));
    assert!(window[FFT_SIZE - 10..].iter().all(|&s| s == 0.25));
}
