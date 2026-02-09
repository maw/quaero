# TODO

## Future Enhancements

- `--max-depth` — limit directory traversal depth
- `-g GLOB` / `--include` — rg-style file filter (limit which files are searched)
- `--exclude` — exclude files matching a pattern

## ignore binary files by default

- They're annoying
- Maybe more to the point, deal better with files that are mostly text but
  contain some garbage.  Running `qae` should not make your terminal beep!

## do something about matches many chars into long lines

- Highlighting

## Speedups

- One of my complaints about `rg` is that, due to its multithreaded nature, it
  presents results in nondeterministic order.  So if we ever go multithread
  we'd need to design things carefully in order to collate and sort before
  presenting results to the user.  At that point, will the speedups from
  multithreading even be worth it?

## deadgrep

- I use deadgrep all the time, can't live without it.  It would be cool if we
  could teach deadgrep to use qae.  I think this likely would involve changing
  qae's output format (optionaly?) as well as some changes to deadgrep.

## change the binary name

- qae isn't a great abbreviation.  qro would be better but it reads a bit like
  a tool for doing things with QR codes.  (It also is the abbreviation for the
  state of Querétaro).
