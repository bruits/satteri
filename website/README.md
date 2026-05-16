# Website

This directory contains code for Sätteri’s website, https://satteri.bruits.org/

The website is built with [Maudit](https://maudit.org/).

## Getting started

Developing the website requires the Maudit CLI. You can install it by running:

```sh
cargo install maudit-cli
```

Also make sure you’ve installed Node.js dependencies too:

```sh
pnpm install
```

You can then run the dev server from this directory:

```sh
cd website
maudit dev
```

The first time you run `maudit dev`, it may be a little slow to compile, but once it completes, you should see a link to a `localhost` URL logged to the terminal.

You can now edit files in the `website/` directory and see the page live update in your browser.
