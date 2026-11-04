use std::hint::black_box;
use std::mem::size_of;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use ginux_desktop_recovery::{Component, Supervisor, RETRY_TICKS};
use ginux_domain_presentation::{Presentation, Rect};

const WIDTH: usize = 1024;
const HEIGHT: usize = 768;
const SAMPLES: usize = 500;
const STARTUP_BUDGET: Duration = Duration::from_millis(50);
const INPUT_TO_FRAME_BUDGET: Duration = Duration::from_micros(16_667);
const FRAME_BUDGET: Duration = Duration::from_micros(16_667);
const RECOVERY_BUDGET: Duration = Duration::from_millis(1);
const WORKING_SET_BUDGET: usize = 8 * 1024 * 1024;

struct Measurement {
    name: &'static str,
    value: u128,
    unit: &'static str,
    budget: u128,
    passed: bool,
}

fn percentile(samples: &mut [Duration], numerator: usize, denominator: usize) -> Duration {
    samples.sort_unstable();
    let index = ((samples.len() - 1) * numerator).div_ceil(denominator);
    samples[index]
}

fn micros(value: Duration) -> u128 {
    value.as_micros()
}

fn presentation(domain: u64) -> Presentation {
    Presentation::start(
        domain,
        Rect::new(0, 0, WIDTH as u16, HEIGHT as u16),
        Rect::new(51, 38, 922, 692),
    )
    .expect("presentation startup")
}

fn main() -> ExitCode {
    let startup_at = Instant::now();
    let mut foreground = presentation(1);
    let mut background = presentation(2);
    foreground.focus().expect("foreground focus");
    let startup = startup_at.elapsed();

    let mut pixels = vec![0_u32; WIDTH * HEIGHT];
    let mut input_samples = Vec::with_capacity(SAMPLES);
    for sample in 1..=SAMPLES {
        let started = Instant::now();
        background.publish(sample as u64).expect("background frame");
        foreground
            .input_round_trip(1, sample as i32, 1)
            .expect("input round trip");
        foreground
            .publish((sample as u64) << 8)
            .expect("foreground frame");
        foreground
            .render(&mut pixels, WIDTH)
            .expect("foreground render");
        black_box(&pixels);
        input_samples.push(started.elapsed());
    }
    let input_p95 = percentile(&mut input_samples, 95, 100);

    let mut frame_samples = Vec::with_capacity(SAMPLES);
    for sample in 1..=SAMPLES {
        foreground
            .publish((sample as u64) << 16)
            .expect("steady frame");
        let started = Instant::now();
        foreground
            .render(&mut pixels, WIDTH)
            .expect("steady render");
        black_box(&pixels);
        frame_samples.push(started.elapsed());
    }
    let frame_p95 = percentile(&mut frame_samples, 95, 100);

    let mut recovery_samples = Vec::with_capacity(SAMPLES);
    let mut supervisor = Supervisor::new();
    for sample in 0..SAMPLES as u64 {
        let component = Component::ALL[sample as usize % Component::ALL.len()];
        let now = sample * 4 + 1;
        let started = Instant::now();
        supervisor.fault(component, now).expect("contained fault");
        supervisor
            .recover(component, now + RETRY_TICKS)
            .expect("bounded recovery");
        black_box(supervisor.snapshot());
        recovery_samples.push(started.elapsed());
    }
    let recovery_p95 = percentile(&mut recovery_samples, 95, 100);

    let modeled_working_set = pixels.len() * size_of::<u32>()
        + size_of::<Presentation>() * 2
        + size_of::<Supervisor>()
        + input_samples.capacity() * size_of::<Duration>()
        + frame_samples.capacity() * size_of::<Duration>()
        + recovery_samples.capacity() * size_of::<Duration>();

    let measurements = [
        Measurement {
            name: "presentation-startup",
            value: micros(startup),
            unit: "us",
            budget: micros(STARTUP_BUDGET),
            passed: startup <= STARTUP_BUDGET,
        },
        Measurement {
            name: "input-to-frame-p95",
            value: micros(input_p95),
            unit: "us",
            budget: micros(INPUT_TO_FRAME_BUDGET),
            passed: input_p95 <= INPUT_TO_FRAME_BUDGET,
        },
        Measurement {
            name: "steady-frame-p95",
            value: micros(frame_p95),
            unit: "us",
            budget: micros(FRAME_BUDGET),
            passed: frame_p95 <= FRAME_BUDGET,
        },
        Measurement {
            name: "recovery-cycle-p95",
            value: micros(recovery_p95),
            unit: "us",
            budget: micros(RECOVERY_BUDGET),
            passed: recovery_p95 <= RECOVERY_BUDGET,
        },
        Measurement {
            name: "modeled-working-set",
            value: modeled_working_set as u128,
            unit: "bytes",
            budget: WORKING_SET_BUDGET as u128,
            passed: modeled_working_set <= WORKING_SET_BUDGET,
        },
    ];

    println!("controller desktop qualification");
    println!("environment=host-release resolution={WIDTH}x{HEIGHT} samples={SAMPLES}");
    println!("metric value unit budget result");
    for measurement in &measurements {
        println!(
            "{} {} {} {} {}",
            measurement.name,
            measurement.value,
            measurement.unit,
            measurement.budget,
            if measurement.passed { "PASS" } else { "FAIL" }
        );
    }

    if measurements.iter().all(|measurement| measurement.passed) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
