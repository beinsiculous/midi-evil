use std::sync::OnceLock;

const BASE_TABLE_LEN: usize = 2_048;
const LEVEL_COUNT: usize = 10;
const MAX_HARMONIC: usize = 64;
const TABLE_COUNT: usize = 31;

struct WaveTable {
    levels: Vec<Vec<f32>>,
}

impl WaveTable {
    fn sample(&self, phase: f32, phase_step: f32) -> f32 {
        let level = level_for_phase_step(phase_step);
        let table = &self.levels[level];
        let position = phase * table.len() as f32;
        let index = position.floor() as usize % table.len();
        let next = (index + 1) % table.len();
        let fraction = position.fract();
        table[index] + (table[next] - table[index]) * fraction
    }
}

static TABLES: OnceLock<Vec<WaveTable>> = OnceLock::new();

pub(crate) fn sample(index: usize, phase: f32, phase_step: f32) -> f32 {
    TABLES.get_or_init(|| (0..TABLE_COUNT).map(build_table).collect())[index]
        .sample(phase, phase_step)
}

fn level_for_phase_step(phase_step: f32) -> usize {
    if phase_step <= 0.0 {
        return 0;
    }
    ((phase_step * BASE_TABLE_LEN as f32).log2().ceil().max(0.0) as usize).min(LEVEL_COUNT - 1)
}

fn build_table(index: usize) -> WaveTable {
    let coefficients = (1..=MAX_HARMONIC)
        .map(|harmonic| (harmonic, coefficient(index, harmonic)))
        .filter(|(_, (a, b))| *a != 0.0 || *b != 0.0)
        .collect::<Vec<_>>();
    let mut levels = (0..LEVEL_COUNT)
        .map(|level| render_table(&coefficients, BASE_TABLE_LEN >> level))
        .collect::<Vec<_>>();
    let normalization = levels
        .iter()
        .flatten()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
        .max(0.000_001);
    for sample in levels.iter_mut().flatten() {
        *sample /= normalization;
    }
    WaveTable { levels }
}

fn render_table(coefficients: &[(usize, (f32, f32))], len: usize) -> Vec<f32> {
    let max_harmonic = (len / 2 - 1).clamp(1, MAX_HARMONIC);
    (0..len)
        .map(|frame| {
            let phase = frame as f32 / len as f32;
            coefficients
                .iter()
                .take_while(|(harmonic, _)| *harmonic <= max_harmonic)
                .map(|(harmonic, (a, b))| {
                    let angle = *harmonic as f32 * phase * 2.0 * std::f32::consts::PI;
                    a * angle.cos() + b * angle.sin()
                })
                .sum::<f32>()
        })
        .collect()
}

fn coefficient(index: usize, harmonic: usize) -> (f32, f32) {
    let n = harmonic as f32;
    match index {
        0..=6 => {
            let b = usize::from(harmonic == index + 2) as f32;
            (0.0, b)
        }
        7..=14 => {
            const WIDTHS: [f32; 8] = [0.1, 0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.9];
            let a = 2.0 / (std::f32::consts::PI * n)
                * (2.0 * std::f32::consts::PI * n * WIDTHS[index - 7]).sin();
            (a, 0.0)
        }
        15..=18 => {
            let allowed = match index {
                16 => harmonic.is_multiple_of(2),
                17 => !harmonic.is_multiple_of(2),
                _ => true,
            };
            let fundamental = if index == 15 && harmonic == 1 {
                2.0
            } else {
                1.0
            };
            let softness = if index == 18 { n } else { 1.0 };
            let b = if allowed {
                -2.0 * fundamental / (std::f32::consts::PI * n * softness)
            } else {
                0.0
            };
            (0.0, b)
        }
        19..=22 => {
            let allowed = if index == 20 {
                harmonic.is_multiple_of(2)
            } else {
                !harmonic.is_multiple_of(2)
            };
            let fundamental = if index == 19 && harmonic == 1 {
                2.0
            } else {
                1.0
            };
            let slope = match index {
                21 => n,
                22 => n * n * n,
                _ => n * n,
            };
            let a = if allowed {
                -8.0 * fundamental / (std::f32::consts::PI.powi(2) * slope)
            } else {
                0.0
            };
            (a, 0.0)
        }
        _ => numerical_coefficient(index, harmonic),
    }
}

fn numerical_coefficient(index: usize, harmonic: usize) -> (f32, f32) {
    let mut a = 0.0;
    let mut b = 0.0;
    for frame in 0..BASE_TABLE_LEN {
        let phase = frame as f32 / BASE_TABLE_LEN as f32;
        let sample = shaped_sample(index, phase);
        let angle = harmonic as f32 * phase * 2.0 * std::f32::consts::PI;
        a += sample * angle.cos();
        b += sample * angle.sin();
    }
    let scale = 2.0 / BASE_TABLE_LEN as f32;
    (a * scale, b * scale)
}

fn shaped_sample(index: usize, phase: f32) -> f32 {
    let sine = (phase * 2.0 * std::f32::consts::PI).sin();
    match index {
        23 => sine.max(0.0),
        24 => sine.abs(),
        25 => sine * sine,
        26 => sine * sine * sine,
        27 => {
            if sine >= 0.0 {
                sine
            } else {
                sine * 0.5
            }
        }
        28 => 1.0 - 4.0 * (phase - 0.5).powi(2),
        29 => {
            let saw = 2.0 * phase - 1.0;
            let triangle = 1.0 - 4.0 * (phase - 0.5).abs();
            (saw + triangle) * 0.5
        }
        30 => {
            let square = if phase < 0.5 { 1.0 } else { -1.0 };
            let triangle = 1.0 - 4.0 * (phase - 0.5).abs();
            (square + triangle) * 0.5
        }
        _ => unreachable!("only shaped waveforms need numerical coefficients"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_finite_normalized_and_band_limited_at_high_rates() {
        for index in 0..TABLE_COUNT {
            for phase_step in [0.0, 0.001, 0.01, 0.1, 0.49] {
                let output = (0..256)
                    .map(|frame| sample(index, (frame as f32 * phase_step).fract(), phase_step))
                    .collect::<Vec<_>>();
                assert!(output.iter().all(|sample| sample.is_finite()));
                assert!(output.iter().all(|sample| sample.abs() <= 1.001));
                if phase_step > 0.0 && phase_step <= 0.01 {
                    assert!(
                        output.iter().any(|sample| sample.abs() > 0.001),
                        "waveform {index} was silent at phase step {phase_step}"
                    );
                }
            }
        }
        assert_eq!(level_for_phase_step(0.49), LEVEL_COUNT - 1);
        assert_eq!(level_for_phase_step(0.0), 0);
    }
}
