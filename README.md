# Monitoring Boi (monboi)

## What is it for?

Monitoring boi contains a mechanism to reliably subscribe to log files, find interesting events and trigger according actions. It is fully event driven and reads logs live as they are written. Both logrotate (the standard Linux log wraparound mechanism) and copy-truncate are supported. Inotify is used to get notified when new data is written to log files. Because of that, *this application only runs on Linux* and only on kernels that support inotify.

Currently, the `/var/log/auth.log` file is monitored for successful login attempts and if someone logs in, an email is dispatched to the configured recipients.  As such, the application is only a proof of concept. However, the log subscriptions can be easily extended. Parsing textual log files is not the most robust method to detect system events, but it is a way that is simple, easily customisable and does not require additional software or libraries.

This application is designed to be deployed on Linux servers that largely run unattended but need light monitoring to ensure secure and reliable operation. The application has a low memory and CPU footprint and is also suited for smaller machines like a Raspberry Pi or a small DigitalOcean droplet.

Please note that a log message may only be generated well after a critical event happened, so this utility is not designed for prevention and is unsuitable for mitigation. It is designed to monitor a system and to keep the administrator informed.

## How to compile

First, install [Rust](https://www.rust-lang.org/tools/install) if you haven't done that already.

Make sure you have the OpenSSL development libraries (required to securely send emails) and dpkg (required by Rust to find OpenSSL) installed. You can do this as follows:

Debian and Ubuntu
```sh
sudo apt-get install pkg-config libssl-dev
```

Arch Linux
```sh
sudo pacman -S pkg-config openssl
```

Fedora
```sh
sudo dnf install pkg-config openssl-devel
```

Build the source code with `cargo`, Rust's package manager.

```sh
cargo build --release
```

It is recommended to build the binary file on your target. It may be possible to build it on one Linux machine and copy it to the other, but if the Linux version or distribution differ by too much (especially the OpenSSL version), it will not run on the second machine.

## How to install & run

The application expects a config file at `/etc/monboi/monboi.conf`. Its format is [TOML](https://github.com/toml-lang/toml) and an example config file can be found in this repository at `service/monboi.conf`. The program will log into the given email account using the SMTP server URL and password provided. It will then send notifications to the given recipient(s). If no configuration file is found, `monboi` exits with an error.

The application can be run manually with:

```sh
sudo target/release/monboi
```

Superuser rights are required to read system log files. If your log files are readable by non-root users, no `sudo` is required.

If you want to install `monboi` as a service, you can use the file `service/monboi` to create a System V init script at `/etc/init.d/monboi`. Make the file executable and owned by root:

```sh
sudo cp service/monboi /etc/init.d/monboi
sudo chmod 755 /etc/init.d/monboi
sudo chown root:root /etc/init.d/monboi
```

Finally, copy the compiled binary file to `/usr/sbin/monboi`:

```sh
sudo cp target/release/monboi /usr/sbin/monboi
```

To see that your service is working, run:

```sh
sudo /etc/init.d/monboi start
sudo /etc/init.d/monboi status
```

Monitoring Boi has been tested on Ubuntu running natively and on Windows in WSL. Additionally, it has been tested on a Raspberry Pi running Raspbian. However, the application should run on all commonly used Linux distributions.

If you want the service to start automatically after a reboot, run the following command:

```sh
sudo update-rc.d monboi defaults
```

To disable autostart again, run:

```sh
sudo update-rc.d monboi disable
```
