# Monitoring Boi (monboi)

## What is it for?

Monitoring boi contains a mechanism to reliably subscribe to log files, find interesting events and trigger according actions. It is fully event driven and reads logs live as they are written. Both logrotate (the standard Linux log wraparound mechanism) and copy-truncate are supported. Inotify is used to get notified when new data is written to log files. Because of that, *this application only runs on Linux* and only on kernels that support inotify.

Currently, the `/var/log/auth.log` file is monitored for successful login attempts and if someone logs in, an email is dispatched to the configured recipients. As such, the application is only a proof of concept. However, the log subscriptions can be easily extended. Parsing textual log files is not the most robust method to detect system events, but it is a way that is simple, easily customisable and does not require additional software or libraries.

This application is designed to be deployed on Linux servers that largely run unattended but need light monitoring to ensure secure and reliable operation. The application has a low memory and CPU footprint and is also suited for smaller machines like a Raspberry Pi 1 or a small DigitalOcean droplet.

Please note that a log message may only be generated well after a critical event happened, so this utility is not designed for prevention and is unsuitable for mitigation. It is designed to monitor a system and to keep the administrator informed.

## How to install

Build the source code with `cargo`, Rust's package manager.

```sh
cargo build --release
```

The application expects a config file at `/etc/monboi/monboi.conf`. Its format is TOML and an example config file can be found in this repository at `service/monboi.conf`. If you want to install the application as a service, please use the file `service/monboi` to create a classic Linux service in `init.d` and copy the binary into a folder that is contained in the `$PATH` variable.