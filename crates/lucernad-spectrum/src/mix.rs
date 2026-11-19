//! Os anéis de amostras por nó e a janela somada para a análise.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::analysis::LOW_FFT_SIZE;

/// Um anel sem amostra há mais que isto conta como silêncio.
pub const STALE: Duration = Duration::from_millis(100);

struct Ring {
    rate: u32,
    samples: VecDeque<f32>,
    last: Instant,
}

/// As últimas `LOW_FFT_SIZE` amostras de cada nó, por `object.serial`, com a
/// taxa de cada um.
#[derive(Default)]
pub struct Rings {
    rings: HashMap<u64, Ring>,
}

impl Rings {
    pub fn new() -> Rings {
        Rings::default()
    }

    /// Acrescenta `mono`, a `rate`, ao anel do nó `serial`, guardando as
    /// últimas `LOW_FFT_SIZE`; uma taxa nova recomeça o anel.
    pub fn push(&mut self, serial: u64, rate: u32, mono: &[f32], now: Instant) {
        let ring = self.rings.entry(serial).or_insert_with(|| Ring {
            rate,
            samples: VecDeque::with_capacity(LOW_FFT_SIZE),
            last: now,
        });
        if ring.rate != rate {
            ring.rate = rate;
            ring.samples.clear();
        }
        ring.last = now;
        let keep = &mono[mono.len().saturating_sub(LOW_FFT_SIZE)..];
        let overflow = (ring.samples.len() + keep.len()).saturating_sub(LOW_FFT_SIZE);
        ring.samples.drain(..overflow);
        ring.samples.extend(keep);
    }

    /// Mantém só os nós de `serials`.
    pub fn retain(&mut self, serials: &[u64]) {
        self.rings.retain(|serial, _| serials.contains(serial));
    }

    /// Esquece todos os nós.
    pub fn clear(&mut self) {
        self.rings.clear();
    }

    /// Os anéis com amostra há no máximo `STALE`.
    fn fresh(&self, now: Instant) -> impl Iterator<Item = (&u64, &Ring)> {
        self.rings
            .iter()
            .filter(move |(_, ring)| now.saturating_duration_since(ring.last) <= STALE)
    }

    /// A taxa da janela: a do anel fresco de menor serial (estável enquanto
    /// ele tocar); `None` sem anel fresco.
    pub fn rate(&self, now: Instant) -> Option<u32> {
        self.fresh(now)
            .min_by_key(|(serial, _)| **serial)
            .map(|(_, ring)| ring.rate)
    }

    /// A soma dos anéis frescos da taxa [`Rings::rate`], alinhada pelo fim,
    /// com `LOW_FFT_SIZE` amostras; um anel velho ou de outra taxa conta zero
    /// (somar taxas diferentes misturaria as frequências).
    pub fn window(&self, now: Instant) -> Vec<f32> {
        let mut sum = vec![0.0; LOW_FFT_SIZE];
        let rate = self.rate(now);
        for (_, ring) in self.fresh(now) {
            if Some(ring.rate) != rate {
                continue;
            }
            let offset = LOW_FFT_SIZE - ring.samples.len();
            for (acc, &s) in sum[offset..].iter_mut().zip(&ring.samples) {
                *acc += s;
            }
        }
        sum
    }
}
