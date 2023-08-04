# fan-controller

PWM fan controller for Orange PI boards made with Rust.

This was just a small personal project to learn [Rust programming language](https://www.rust-lang.org/) and to have a fan controller for my board.

## Installation

This program depends on [wiringOP](https://github.com/orangepi-xunlong/wiringOP) so please install it first.

```sh
git clone https://github.com/orangepi-xunlong/wiringOP.git
cd wiringOP
./build clean
./build
```

Then build and install this application.

```sh
cargo build --release
cp target/release/fan-controller /usr/local/bin
```

Please see the provided help for how to use the application.

```sh
fan-controller --help
```

### Systemd

To use this as a service with systemd enabled systems, please follow steps shown below.

```sh
fan-controller --gpio-pwn 3 --print-systemd > /etc/systemd/system/fan-controller.service
systemctl daemon-reload
systemctl start fan-controller.service
```

Check from logs that the service actually works as you intended it to work.

```sh
journalctl -u fan-controller.service --since="5 minutes ago"
```

Then enable the service to run during system startup.

```sh
systemctl enable fan-controller.service
```

## Testing

```sh
cargo test
```
