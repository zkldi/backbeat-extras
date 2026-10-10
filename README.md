# Extra Backbeat Tools

Extra tools I wrote for [backbeat](https://backbeat.ac). These are not part of the backbeat standard nor necessarily, but mostly just useful stuff you can build ontop of it.

# Packager

The packager is a tool that takes rhythm game formats and turns them into `.bb` files. You can download it [here](todo).

This is my personal packager, and supports only some rhythm game formats. Write your own, if you need something else. It's easy!

# CLI

The Backbeat CLI combines a lot of tools I've written in a useful way, and provides command line access to `backbeat_sdk`s myriad functions.

You can install it like this:

```sh
cargo install bkb
bkb info
```

The backbeat CLI combines all the other tools here, you can do `bkb guts pack`, `bkb start-data-server` etc.

The backbeat CLI is also available via docker: `docker pull ghcr.io/zkldi/backbeat-cli:latest`.

# Server

This is an implementation of a backbeat server that uses your installed charts. Using it via the CLI (`bkb start-data-server`) is easiest.

# Inspector

The inspector is a tool that can look at a `.bb` file and tell you things about the chart and its dependencies.

# VF

The virtual folder thing is an experiment using `FUSE` to project your backbeat store as if it was an ordinary folder. This is advanced stuff, and you should only dig around in here if you know what `FUSE` means and you correctly realise how funny this is.
