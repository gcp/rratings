# rratings (Rust Ratings)

rratings is a tool meant to be run over a set of compressed chess game PGNs
to recalculate the ratings for all players, and determine the accuracy of
various rating systems.

It is geared specifically towards the lichess dataset:
https://database.lichess.org/

Which is both huge and free.

This is not meant to be a production tool. It's meant to assist research
into practical rating systems.

## Usage

1) Download a database of PGNs with game results. The code assumes that
the games inside each PGN are approximately ordered by date & time. This is
true for the above dataset.
2) bunzip2 the PGNs and recompress them with zstd. You want this because
bunzip2 is rather slow to decompress and will limit the performance of the
tool, especially if you're going to be running it multiple times. Alternatively,
hack the source to support bzip2 directly.
3) Change the paths in main() to point to your files.
4) cargo run --release

The tool assumes that sorting the files gets them in date order (again, true
for the lichess dataset) and will process them one by one. Ratings are kept
between files, but the prediction accuracy is reset after every file. This means
that you can use older files to seed historical ratings and then measure the
prediction performance over (say) a month.

## Note

By default only blitz games are considered. These have the largest pool of
players in lichess.

Because we only need the game headers of the PGN, you can filter the PGNs
through pgn-extract with some combination of --plylimit 0 and -R to throw
away the unneeded moves and tags. This vastly speeds up traversing the DB.

## To Do

* Extract rating parameters into class constructors.
* Run a global optimizer to find the optimal parameters.

## License

MIT
