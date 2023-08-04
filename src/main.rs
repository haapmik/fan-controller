use clap::Parser;
use libc::c_int;
use std::{fs, path::Path, thread, time};

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

    #[arg(long, default_value_t = 2)]
    pwm_increment: i32,

    #[arg(long, default_value_t = 1)]
    pwm_decrement: i32,

    /// Target temperature to maintain
    #[arg(short, long, default_value_t = 40.0)]
    temperature_target_value: f32,

    // Max allowed temperature value
    #[arg(long, default_value_t = 70.0)]
    temperature_max_value: f32,

    #[arg(long, default_value = "/sys/class/thermal/thermal_zone0/temp")]
    temperature_file_path: String,

    /// Temperature polling rate
    #[arg(short, long, default_value_t = 5)]
    pollrate: u64,

    // GPIO pin controlling the fan
    #[arg(short, long)]
    gpio_pwm: i32,
}

struct Pwm {
    current: i32,
    previous: i32,
    increment: i32,
    decrement: i32,
    min: i32,
    max: i32,
    gpio_pin: i32,
}

impl Pwm {
    fn new(args: &Args) -> Self {
        Self {
            current: args.pwm_max,
            previous: args.pwm_max,
            increment: args.pwm_increment,
            decrement: args.pwm_decrement,
            min: args.pwm_min,
            max: args.pwm_max,
            gpio_pin: args.gpio_pwm,
        }
    }

    fn init(&self) {
        unsafe {
            wiringPiSetup();
            pinMode(self.gpio_pin, 1); // 1 = output
            softPwmCreate(self.gpio_pin, self.max, self.max); // GPIO pin, initial value, range
        }
    }

    fn fix_pwm_value(&self, value: i32) -> i32 {
        if value > self.max {
            return self.max;
        }

        if value < self.min {
            return self.min;
        }

        return value;
    }

    fn write(&mut self, value: i32) {
        self.previous = self.current;
        self.current = self.fix_pwm_value(value);
        unsafe {
            softPwmWrite(self.gpio_pin, self.current);
        }
    }
}

struct Temperature {
    current: f32,
    previous: f32,
    max: f32,
    target: f32,
    source_file_path: String,
}

impl Temperature {
    fn new(args: &Args) -> Self {
        Self {
            current: 0.0,
            previous: 0.0,
            max: args.temperature_max_value,
            target: args.temperature_target_value,
            source_file_path: args.temperature_file_path.to_string(),
        }
    }

    fn read(&mut self) {
        let path = Path::new(&self.source_file_path);
        let fcontext = fs::read_to_string(path).unwrap_or_else(|error| {
            panic!(
                "Failed to read temperature from {:?}: {:?}",
                path.display(),
                error
            );
        });

        let value: f32 = fcontext.trim().parse().unwrap_or_else(|error| {
            panic!("Failed to parse temperature value: {:?}", error);
        });

        self.previous = self.current;

        // Round to one decimal point
        self.current = ((value / 1000.0) * 10.0).round() / 10.0;
    }
}

struct Controller {
    pollrate: time::Duration,
    temperature: Temperature,
    pwm: Pwm,
}

impl Controller {
    fn new(args: &Args) -> Self {
        Self {
            pollrate: time::Duration::from_secs(args.pollrate),
            temperature: Temperature::new(&args),
            pwm: Pwm::new(&args),
        }
    }

    fn get_required_pwm(&self) -> i32 {
        if self.temperature.current >= self.temperature.max {
            return self.pwm.max;
        }

        if self.temperature.current > self.temperature.target
            && self.temperature.previous <= self.temperature.current
        {
            return self.pwm.current + self.pwm.increment;
        }

        if self.temperature.current > self.temperature.target
            && self.temperature.previous > self.temperature.current
        {
            return self.pwm.current - self.pwm.decrement;
        }

        if self.temperature.current < self.temperature.target {
            return self.pwm.current - self.pwm.decrement;
        }

        return self.pwm.current;
    }

    fn start(&mut self) {
        self.pwm.init();

        loop {
            thread::sleep(self.pollrate);

            self.temperature.read();

            // Avoid making unnecessary PWM changes when we are near the target temperature
            if self.temperature.current.round() == self.temperature.target {
                continue;
            }

            let new_pwm = self.get_required_pwm();

            if new_pwm > self.pwm.current {
                self.pwm.write(new_pwm);
                println!(
                    "Current temperature {}°C (target {}°C), rising fan speed {} -> {}",
                    self.temperature.current,
                    self.temperature.target,
                    self.pwm.previous,
                    self.pwm.current
                );
            }

            if new_pwm < self.pwm.current {
                self.pwm.write(new_pwm);
                println!(
                    "Current temperature {}°C (target {}°C), lowering fan speed {} -> {}",
                    self.temperature.current,
                    self.temperature.target,
                    self.pwm.previous,
                    self.pwm.current
                );
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let mut controller = Controller::new(&args);
    controller.start();
}

#[cfg(test)]
mod tests {
    use super::{Controller, Pwm, Temperature};
    use std::time;

    #[test]
    fn pwm_value_too_high() {
        let pwm = Pwm {
            current: 0,
            previous: 0,
            increment: 2,
            decrement: 1,
            min: 0,
            max: 100,
            gpio_pin: 0,
        };

        let pwm_value = pwm.max + 10;
        let value = pwm.fix_pwm_value(pwm_value);
        assert_eq!(pwm.max, value);
    }

    #[test]
    fn pwm_value_too_low() {
        let pwm = Pwm {
            current: 0,
            previous: 0,
            increment: 2,
            decrement: 1,
            min: 0,
            max: 100,
            gpio_pin: 0,
        };

        let pwm_value = pwm.min - 10;
        let value = pwm.fix_pwm_value(pwm_value);
        assert_eq!(pwm.min, value);
    }

    #[test]
    fn pwm_value_within_limits() {
        let pwm = Pwm {
            current: 0,
            previous: 0,
            increment: 2,
            decrement: 1,
            min: 0,
            max: 100,
            gpio_pin: 0,
        };

        let pwm_value = pwm.max - 10;
        let value = pwm.fix_pwm_value(pwm_value);
        assert_eq!(pwm_value, value);
    }

    #[test]
    fn temperature_over_high_limit() {
        let controller = Controller {
            pollrate: time::Duration::from_secs(5),
            temperature: Temperature {
                max: 70.0,
                current: 80.0, // Higher than max
                previous: 0.0,
                target: 40.0,
                source_file_path: "".to_string(),
            },
            pwm: Pwm {
                current: 0,
                previous: 0,
                decrement: 1,
                increment: 2,
                min: 0,
                max: 100,
                gpio_pin: 0,
            },
        };

        let value = controller.get_required_pwm();
        assert_eq!(controller.pwm.max, value);
    }

    #[test]
    fn temperature_same_as_target() {
        let controller = Controller {
            pollrate: time::Duration::from_secs(5),
            temperature: Temperature {
                target: 40.0,
                current: 40.0, // Same as target
                previous: 0.0,
                max: 70.0,
                source_file_path: "".to_string(),
            },
            pwm: Pwm {
                current: 50,
                previous: 0,
                decrement: 1,
                increment: 2,
                min: 0,
                max: 100,
                gpio_pin: 0,
            },
        };

        let value = controller.get_required_pwm();
        assert_eq!(controller.pwm.current, value);
    }

    #[test]
    fn temperature_over_target_and_rising() {
        let controller = Controller {
            pollrate: time::Duration::from_secs(5),
            temperature: Temperature {
                target: 40.0,
                current: 55.0,  // Higher than target and previous
                previous: 50.0, // Lower than current
                max: 70.0,
                source_file_path: "".to_string(),
            },
            pwm: Pwm {
                current: 50,
                previous: 0,
                decrement: 1,
                increment: 2,
                min: 0,
                max: 100,
                gpio_pin: 0,
            },
        };

        let value = controller.get_required_pwm();
        assert_eq!(controller.pwm.current + controller.pwm.increment, value);
    }

    #[test]
    fn temperature_over_target_and_lowering() {
        let controller = Controller {
            pollrate: time::Duration::from_secs(5),
            temperature: Temperature {
                target: 40.0,
                current: 50.0,  // Higher than target, but lower than previous
                previous: 55.0, // Higher than current
                max: 70.0,
                source_file_path: "".to_string(),
            },
            pwm: Pwm {
                current: 50,
                previous: 0,
                decrement: 1,
                increment: 2,
                min: 0,
                max: 100,
                gpio_pin: 0,
            },
        };

        let value = controller.get_required_pwm();
        assert_eq!(controller.pwm.current - controller.pwm.decrement, value);
    }

    #[test]
    fn temperature_below_target() {
        let controller = Controller {
            pollrate: time::Duration::from_secs(5),
            temperature: Temperature {
                target: 40.0,
                current: 30.0, // Lower than target
                previous: 0.0,
                max: 70.0,
                source_file_path: "".to_string(),
            },
            pwm: Pwm {
                current: 50,
                previous: 0,
                decrement: 1,
                increment: 2,
                min: 0,
                max: 100,
                gpio_pin: 0,
            },
        };

        let value = controller.get_required_pwm();
        assert_eq!(controller.pwm.current - controller.pwm.decrement, value);
    }
}
