# scooter

Scooter is an interactive find-and-replace terminal UI app.

Search with either a fixed string or a regular expression, enter a replacement, and interactively toggle which instances you want to replace. You can also specify a regex pattern for the file paths you want to search.

If the instance you're attempting to replace has changed since the search was performed, e.g. if you've switched branches and that line no longer exists, that particular replacement won't occur: you'll see all such cases at the end.

![Scooter preview](media/preview.gif)

## Installation

### Cargo

Ensure you have cargo installed (see [here](https://doc.rust-lang.org/cargo/getting-started/installation.html)), and then run

```sh
cargo install scooter
```

### Building from source

Ensure you have cargo installed (see [here](https://doc.rust-lang.org/cargo/getting-started/installation.html)), then pull down the repo and run

```sh
cargo install --path .
```

## Usage

Run `scooter` in a terminal to launch Scooter. You can then enter some text to search with and text to replace matches with, toggle on or off fixed strings, and enter a regex pattern that filenames must match. A more extensive set of keymappings will be shown at the bottom of the window: these vary slightly depending on the screen you're on.
