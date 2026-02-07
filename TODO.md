# TODO

## Group B — Future Enhancements

- `--max-depth` — limit directory traversal depth
- `-g GLOB` / `--include` — rg-style file filter (limit which files are searched)
- `--exclude` — exclude files matching a pattern

## Speedups

- One of my complaints about `rg` is that, due to its multithreaded nature, it
  presents results in nondeterministic order.  So if we ever go multithread
  we'd need to design things carefully in order to collate and sort before
  presenting results to the user.  At that point, will the speedups from
  multithreading even be worth it?
