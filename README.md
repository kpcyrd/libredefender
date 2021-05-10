# libredefender

Imagine the information security compliance guideline says you need an antivirus but you run Arch Linux.

libredefender is an antivirus program featuring:

- **Industry standards** - Scanning is implemented with libclamav
- **Signatures** - We have that
- **Scheduling** - Starts scans periodically and lets you know if there's something to do

The process is trying to change both io and processor priority to idle.

`clamav-freshclam.service` needs to be setup.

## Example config

```toml
[scan]
excludes = [
    # rust build folders
    "/home/user/repos/**/target",
]
skip_hidden = true
skip_larger_than = "30MiB"

[update]
# use data fetched by clamav-freshclam.service (default)
path = "/var/lib/clamav"

[schedule]
```

## License

GPLv3+
