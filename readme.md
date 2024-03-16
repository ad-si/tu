# `tu`

CLI tool to convert a natural language date/time **t**o **U**TC.


## Installation

```sh
cargo install tu
```


## Usage

```txt
Usage: tu <natural time/duration>

Examples:
  tu today      -> 2024-03-16T12:56:41.905455Z
  tu tomorrow   -> 2024-03-17T12:56:41.905455Z
  tu 2 day      -> 2024-03-18T12:56:41.905455Z
  tu 9 week     -> 2024-05-18T12:56:41.905455Z
  tu 1 month    -> 2024-04-16T00:00:00Z
```

This is especially useful in combination with other tools like [TaskLite]:

```bash
# Bash
tasklite add "Buy bike" due:$(tu 2 week)
```

```fish
# Fish
tasklite add "Buy bike" due:(tu 2 week)
```

[TaskLite]: https://tasklite.org
