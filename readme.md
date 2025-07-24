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

  tu 2024-04-10T13:31:46+04:00     -> 2024-04-10T09:31:46Z
  tu Wed, 14 Feb 2024 23:16:09 GMT -> 2024-02-14T23:16:09Z
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


## Related

- [`biff`] - CLI tool for datetime arithmetic, parsing, formatting, etc.
- [dateutils] - CLI date and time utilities for calculations and conversions
- [`fuzzy-time`] - Haskell package for parsing fuzzy time strings.
- [utcify.ad-si.com] - Web tool to convert local-time strings to UTC.
- [uutils `date`] - Print or set the system date and time.
- [`when`] - Timezone CLI tool.

[`biff`]: https://github.com/BurntSushi/biff
[dateutils]: https://github.com/hroptatyr/dateutils
[`fuzzy-time`]: https://github.com/NorfairKing/fuzzy-time
[utcify.ad-si.com]: https://utcify.ad-si.com
[uutils `date`]: https://uutils.github.io/coreutils/docs/utils/date.html
[`when`]: https://github.com/mitsuhiko/when
