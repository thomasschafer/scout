# scooter

Scooter is an interactive find-and-replace terminal UI app.

Search with either a fixed string or a regular expression, enter a replacement, and interactively toggle which instances you want to replace. You can also specify a regex pattern for the file paths you want to search.

If the instance you're attempting to replace has changed since the search was performed, e.g. if you've switched branches and that line no longer exists, that particular replacement won't occur: you'll see all such cases at the end.

![Scooter preview](media/preview.gif)

## Features

Scooter respects both `.gitignore` and `.ignore` files.

You can add capture groups to the search regex and use them in the replacement string: for instance, if you use `(\d) - (\w+)` for the search text and `($2) "$1"` as the replacement, then `9 - foo` would be replaced with `(foo) "9"`.

## Usage

Run

```sh
scooter
```

in a terminal to launch Scooter. By default the current directory is used to search and replace in, but you can pass in a directory as the first argument to override this behaviour:

```sh
scooter ../foo/bar`
```

You can then enter some text to search with and text to replace matches with, toggle on or off fixed strings, and enter a regex pattern that filenames must match. A more extensive set of keymappings will be shown at the bottom of the window: these vary slightly depending on the screen you're on.

## Installation

### Cargo

Ensure you have cargo installed (see [here](https://doc.rust-lang.org/cargo/getting-started/installation.html)), then run:

```sh
cargo install scooter
```

### Prebuilt binaries

You can download binaries from the [releases page](https://github.com/thomasschafer/scooter/releases/latest). After downloading, unzip the binary and move it to a directory in your `PATH`.

- **Linux**
  - Intel/AMD: `*-x86_64-unknown-linux-musl.tar.gz`
  - ARM64: `*-aarch64-unknown-linux-musl.tar.gz`
- **macOS**
  - Apple silicon: `*-aarch64-apple-darwin.tar.gz`
  - Intel: `*-x86_64-apple-darwin.tar.gz`
- **Windows**
  - `*-x86_64-pc-windows-msvc.zip`

### Building from source

Ensure you have cargo installed (see [here](https://doc.rust-lang.org/cargo/getting-started/installation.html)), then run the following commands:

```sh
git clone git@github.com:thomasschafer/scooter.git
cd scooter
cargo install --path .
```

## Contributing

Contributions are very welcome! I'd be especially grateful for any contributions to add scooter to popular package managers. If you'd like to add a new feature, please create an issue first so we can discuss the idea, then create a PR with your changes.
