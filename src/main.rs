use clap::Parser;
use libc::c_int;
use std::{fs, thread, time};

#[link(name = "wiringPi")]
extern "C" {
    fn wiringPiSetup() -> c_int;
    fn pinMode(pin: c_int, mode: c_int);
    fn softPwmCreate(pin: c_int, value: c_int, range: c_int) -> c_int;
    fn softPwmWrite(pin: c_int, value: c_int);
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Minimum allowed fan speed
    #[arg(long, default_value_t = 30)]
    pwm_min: i32,

    /// Maximum allowed fan speed
    #[arg(long, default_value_t = 100)]
    pwm_max: i32,

    /// Target temperature to maintain
    #[arg(short, long, default_value_t = 40)]
    temperature_target_value: i32,

    // Max allowed temperature value
    #[arg(long, default_value_t = 70)]
    temperature_max_value: i32,

    #[arg(long, default_value = "/sys/class/thermal/thermal_zone0/temp")]
    temperature_file_path: String,

    /// Temperature polling rate
    #[arg(short, long, default_value_t = 5)]
    pollrate: u64,

    // GPIO pin controlling the fan
    #[arg(short, long)]
    gpio_pwm: i32,
}

fn read_temperature(file_path: &str) -> Result<i32, std::io::Error> {
    let fcontent = fs::read_to_string(file_path)?;
    let value: i32 = fcontent.trim().parse().unwrap_or_else(|error| {
        panic!("Failed to read temperature: {:?}", error);
    });
    Ok(value)
}

fn fix_pwm_value(value: i32, args: &Args) -> i32 {
    if value > args.pwm_max {
        return args.pwm_max;
    }

    if value < args.pwm_min {
        return args.pwm_min;
    }

    return value;
}

fn check_required_pwm(
    current_temperature: i32,
    previous_temperature: i32,
    current_pwm: i32,
    args: &Args,
) -> i32 {
    if current_temperature == args.temperature_target_value {
        return current_pwm;
    }

    if current_temperature >= args.temperature_max_value {
        return args.pwm_max;
    }

    if current_temperature > args.temperature_target_value
        && previous_temperature <= current_temperature
    {
        return current_pwm + 2;
    }

    if current_temperature > args.temperature_target_value
        && previous_temperature > current_temperature
    {
        return current_pwm - 1;
    }

    if current_temperature < args.temperature_target_value {
        return current_pwm - 1;
    }

    return current_pwm;
}

fn write_pwm_value(value: i32, gpio_pin: i32) {
    unsafe {
        softPwmWrite(gpio_pin, value);
    }
}

fn controller(args: &Args) {
    let mut old_pwm = args.pwm_min;
    let mut previous_temperature: i32 = 0;

    let sleep_time = time::Duration::from_secs(args.pollrate);

    loop {
        thread::sleep(sleep_time);

        let current_temperature = match read_temperature(&args.temperature_file_path) {
            Ok(value) => value,
            Err(error) => {
                // On error, raise fan speed to max to avoid damage
                write_pwm_value(args.pwm_max, args.gpio_pwm);
                panic!("Controller failed: {:?}", error)
            }
        };

        if current_temperature == args.temperature_target_value {
            continue;
        }

        let new_pwm = fix_pwm_value(
            check_required_pwm(current_temperature, previous_temperature, old_pwm, args),
            &args,
        );

        if new_pwm > old_pwm {
            println!(
                "Current temperature {}°C (target {}°C), rising fan speed {} -> {}",
                current_temperature, args.temperature_target_value, old_pwm, new_pwm
            );
            write_pwm_value(new_pwm, args.gpio_pwm);
        }

        if new_pwm < old_pwm {
            println!(
                "Current temperature {}°C (target {}°C), lowering fan speed {} -> {}",
                current_temperature, args.temperature_target_value, old_pwm, new_pwm
            );
            write_pwm_value(new_pwm, args.gpio_pwm);
        }

        old_pwm = new_pwm;
        previous_temperature = current_temperature;
    }
}

fn initialize(args: &Args) {
    unsafe {
        wiringPiSetup();
        pinMode(args.gpio_pwm, 1); // 1 = output
        softPwmCreate(args.gpio_pwm, args.pwm_max, args.pwm_max); // GPIO pin, initial value, range
    }
}

fn main() {
    let args = Args::parse();
    initialize(&args);
    controller(&args);
}

#[cfg(test)]
mod tests {
    use super::{check_required_pwm, fix_pwm_value, Args};

    const ARGS: Args = Args {
        pwm_max: 100,
        pwm_min: 0,
        temperature_target_value: 50,
        temperature_max_value: 70,
        temperature_file_path: String::new(),
        pollrate: 0,
        gpio_pwm: 0,
    };

    #[test]
    fn pwm_value_too_high() {
        let pwm_value = ARGS.pwm_max + 10;
        let value = fix_pwm_value(pwm_value, &ARGS);
        assert_eq!(ARGS.pwm_max, value);
    }

    #[test]
    fn pwm_value_too_low() {
        let pwm_value = ARGS.pwm_min - 10;
        let value = fix_pwm_value(pwm_value, &ARGS);
        assert_eq!(ARGS.pwm_min, value);
    }

    #[test]
    fn pwm_value_within_limits() {
        let pwm_value = ARGS.pwm_max - 10;
        let value = fix_pwm_value(pwm_value, &ARGS);
        assert_eq!(pwm_value, value);
    }

    #[test]
    fn temperature_over_high_limit() {
        let curr_temperature = ARGS.temperature_max_value + 1;
        let prev_temperature = 0; // shouldn't matter
        let pwm = 50;
        let value = check_required_pwm(curr_temperature, prev_temperature, pwm, &ARGS);
        assert_eq!(ARGS.pwm_max, value);
    }

    #[test]
    fn temperature_same_as_target() {
        let curr_temperature = ARGS.temperature_target_value;
        let prev_temperature = 0; // shouldn't matter
        let pwm = 50;
        let value = check_required_pwm(curr_temperature, prev_temperature, pwm, &ARGS);
        assert_eq!(pwm, value);
    }

    #[test]
    fn temperature_over_target_and_rising() {
        let curr_temperature = ARGS.temperature_target_value + 10;
        let prev_temperature = curr_temperature - 5;
        let pwm = 50;
        let value = check_required_pwm(curr_temperature, prev_temperature, pwm, &ARGS);
        assert_eq!(pwm + 2, value);
    }

    #[test]
    fn temperature_over_target_and_lowering() {
        let curr_temperature = ARGS.temperature_target_value + 10;
        let prev_temperature = curr_temperature + 5;
        let pwm = 50;
        let value = check_required_pwm(curr_temperature, prev_temperature, pwm, &ARGS);
        assert_eq!(pwm - 1, value);
    }

    #[test]
    fn temperature_below_target() {
        let curr_temperature = ARGS.temperature_target_value - 10;
        let prev_temperature = 0; // shouldn't matter
        let pwm = 50;
        let value = check_required_pwm(curr_temperature, prev_temperature, pwm, &ARGS);
        assert_eq!(pwm - 1, value);
    }
}
