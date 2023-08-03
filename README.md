# fan-controller
PWM Fan controller for Orange PI system made with Rust.

This was just a small personal project to learn [Rust programming language](https://www.rust-lang.org/).

## Installation

This program depends on [wiringOP](https://github.com/orangepi-xunlong/wiringOP) so please install it first.

```sh
git clone https://github.com/orangepi-xunlong/wiringOP.git
cd wiringOP
./build clean
./build
```

Then compile this project.

```sh
cargo build --release
```
