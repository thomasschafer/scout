# scout

Scout is an interactive find-and-replace terminal UI app.

Search with either a fixed string or a regular expression, enter a replacement, and interactively toggle which instances you want to replace. You can also specify a regex pattern for the file paths you want to search.

If the instance you're attempting to replace has changed since the search was performed, e.g. if you've switched branches and that line no longer exists, that particular replacement won't occur: you'll see all such cases at the end.

![Scout preview](media/preview.gif)

## Installation

Install with

```sh
cargo install --path .
```

then run `scout` from the directory you want to search in.
