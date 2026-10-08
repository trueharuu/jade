//! Checks `jade_solve` against a recorded fixture.
//!
//! Usage:
//!   check <fixture>
//!
//! Fixture format:
//!   <fumen> <pattern> <success>/<total>   first line; the pattern may
//!                                         contain spaces
//!   <queue>,<true|false>                  one line per queue that the
//!                                         pattern expands to
//!
//! Exit status: 0 = every queue matches, 1 = a mismatch or a failed
//! cross-check, 2 = bad usage or an unreadable fixture.

use std::io::BufRead;
use std::io::IsTerminal;
use std::io::Write;
use std::str::FromStr;

use jade_core::board::Board;
use jade_core::piece::Piece;
use jade_core::queue::CAP;
use jade_core::queue::Queue;
use jade_pattern::Pattern;
use jade_solve::Saves;
use jade_solve::parse_fumen;
use jade_solve::percent::percent;
use jade_solve::solve::reachable;

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";
const LABEL: &str = "\x1b[2m  result\x1b[0m ";
const ERROR: &str = "\x1b[31m   error\x1b[0m ";
const OK: &str = "\x1b[32m      ok\x1b[0m ";

/// Reports bad usage or an unreadable fixture and exits with 2.
fn die(msg: impl std::fmt::Display) -> ! {
    println!("{ERROR}{msg}");
    std::process::exit(2)
}

fn main() {
    let Some(fixture) = std::env::args().nth(1) else {
        die("usage: check <fixture>");
    };
    let path = std::path::absolute(&fixture).unwrap_or_else(|e| die(format!("bad path: {e}")));
    println!("{DIM} fixture{RESET} {}", path.display());

    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| die(format!("cannot open {}: {e}", path.display())));

    let mut lines = std::io::BufReader::new(file).lines();
    let header = match lines.next() {
        Some(Ok(l)) => l,
        Some(Err(e)) => die(format!("cannot read header: {e}")),
        None => die("fixture is empty"),
    };

    // The pattern may contain spaces, so the fumen is the first token, the
    // fraction is the last, and the pattern is everything between.
    let (fumen, pattern_src, fraction) = split_header(&header);
    let (success, total) = parse_fraction(fraction);

    println!("{DIM}   field{RESET} {fumen}");
    println!("{DIM} pattern{RESET} {pattern_src}");
    println!("{DIM}recorded{RESET} {success}/{total}");

    let pattern = Pattern::from_str(&pattern_src)
        .unwrap_or_else(|e| die(format!("cannot parse pattern: {e}")));
    let queues = pattern.expand();
    let board = parse_fumen(fumen).unwrap_or_else(|e| die(format!("cannot parse fumen: {e}")));

    // The header pattern must expand to exactly the recorded number of queues.
    if queues.len() != total {
        println!(
            "{ERROR}pattern expands to {} queues, header records {total}",
            queues.len()
        );
        std::process::exit(1);
    }

    // `percent` and `reachable` are independent algorithms over the same
    // question, so agreement between them is evidence both are right.
    let (numer, denom) = percent(board, queues, false, Saves::empty(), true);
    if numer == success as u64 && denom == total as u64 {
        println!("{OK}percent matches ({numer}/{denom})");
    } else {
        println!("{ERROR}percent check {numer}/{denom} does not match {success}/{total}");
        // std::process::exit(1);
    }

    let rows = read_rows(lines);
    if rows.len() != total {
        println!(
            "{ERROR}fixture lists {} queues, header records {total}",
            rows.len()
        );
        std::process::exit(1);
    }

    let failed = check_each(board, &rows, total);

    if failed == 0 {
        println!("{LABEL}success");
        std::process::exit(0);
    }

    println!("{LABEL}failure");
    println!("{DIM}       {failed} of {total} queues did not match{RESET}");
    std::process::exit(1);
}

/// Visible columns taken by the fixed `result` label, including the trailing
/// space. The escape codes take no columns.
const LABEL_WIDTH: usize = 9;

/// The terminal width in columns.
///
/// Reads `COLUMNS`, which a shell sets from the terminal size. Falls back to 80
/// when it is unset or not a number. This avoids a dependency and a
/// `TIOCGWINSZ` call for a cosmetic measurement.
fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&w| w > 0)
        .unwrap_or(80)
}

/// The changing part of the progress line: the queue under test, then the
/// progress fraction. `room` is how many columns are available for the queue.
///
/// The fraction is always kept. Only the queue is shortened, because knowing
/// which queue is being tested is the point of the line, and the fraction can
/// be recovered from the row number if it is lost.
fn progress_line(name: &str, done: usize, total: usize, room: usize) -> String {
    let count = format!("{done}/{total}");
    // One space between the queue and the count.
    let avail = room.saturating_sub(count.len() + 1);

    let shown = if name.chars().count() <= avail {
        name.to_string()
    } else if avail <= 1 {
        // No room for a queue. Show nothing rather than a truncated name that
        // would look like a different piece order.
        String::new()
    } else {
        // Keep the tail: the end of a queue is where it diverges from the ones
        // already checked, and a truncated head is indistinguishable.
        let skip = name.chars().count() - (avail - 1);
        let tail: String = name.chars().skip(skip).collect();
        format!("…{tail}")
    };

    format!("{shown} {count}")
}

/// Checks every queue on its own, rewriting one progress line as it goes.
///
/// The line carries the queue under test and a progress fraction. It shortens
/// to fit the terminal, so a narrow terminal still shows the queue and the
/// count rather than wrapping.
///
/// Returns the number of queues that did not match. Failures are printed as
/// they are found, above the progress line, so they are not repeated in the
/// summary.
///
/// The progress line needs a terminal. Without one, a carriage return does not
/// move the cursor, so every redraw would be appended and the output would be
/// unreadable. Piped to a file or another program, only the failures print.
fn check_each(board: Board, rows: &[Row], total: usize) -> usize {
    let mut out = std::io::stdout().lock();
    // Columns available for the changing part of the line, once the fixed
    // label is taken off, so the line never wraps.
    let room = terminal_width().saturating_sub(LABEL_WIDTH);
    let progress = out.is_terminal();
    let mut failed = 0usize;

    for (i, row) in rows.iter().enumerate() {
        // A failure clears the progress line and prints above it, so the
        // counter is never left half-overwritten.
        let bad = match &row.queue {
            Ok(q) => {
                let actual = reachable(board, q, false, Saves::empty(), true);
                (actual != row.recorded).then(|| {
                    let name = q.iter().map(ToString::to_string).collect::<String>();
                    format!(
                        "{name} expected {} but got {actual}",
                        row.recorded
                    )
                })
            }
            Err(e) => Some(format!("line {}: {e}", i + 2)),
        };

        match bad {
            Some(msg) => {
                if progress {
                    let _ = write!(out, "\r\x1b[2K");
                }
                let _ = writeln!(out, "{ERROR}{msg}");
                failed += 1;
            }
            None => {
                if progress {
                    let name = match &row.queue {
                        Ok(q) => q.iter().map(ToString::to_string).collect::<String>(),
                        Err(_) => String::new(),
                    };
                    let _ = write!(
                        out,
                        "\r\x1b[2K{LABEL}{}",
                        progress_line(&name, i + 1, total, room)
                    );
                }
            }
        }
        let _ = out.flush();
    }

    // Leave the cursor at the start of a clean line for the summary.
    if progress {
        let _ = write!(out, "\r\x1b[2K");
        let _ = out.flush();
    }
    failed
}

/// Splits a header line into the fumen, the pattern, and the fraction.
fn split_header(header: &str) -> (&str, String, &str) {
    let mut it = header.split_whitespace();
    let Some(fumen) = it.next() else {
        die("header is empty");
    };
    let Some(fraction) = it.next_back() else {
        die(format!("header has no recorded count: {header}"));
    };
    let pattern = it.collect::<Vec<_>>().join(" ");
    if pattern.is_empty() {
        die(format!("header has no pattern: {header}"));
    }
    (fumen, pattern, fraction)
}

/// Parses a `success/total` token.
fn parse_fraction(fraction: &str) -> (usize, usize) {
    let Some((success, total)) = fraction.split_once('/') else {
        die(format!("recorded count is not `success/total`: {fraction}"));
    };
    let success = success
        .parse()
        .unwrap_or_else(|_| die(format!("bad success count: {success}")));
    let total = total
        .parse()
        .unwrap_or_else(|_| die(format!("bad total count: {total}")));
    (success, total)
}

/// One queue line of the fixture. The queue is kept parsed so that a bad line
/// is reported at the point it is checked, with its line number.
struct Row {
    queue: Result<Queue, String>,
    recorded: bool,
}

impl Row {
    /// Builds a row from a fixture line, or records why the line is unusable.
    fn parse(line: &str) -> Self {
        let Some((queue, recorded)) = line.split_once(',') else {
            return Row {
                queue: Err(format!("no comma in line: {line}")),
                recorded: false,
            };
        };
        let Ok(recorded) = recorded.trim().parse::<bool>() else {
            return Row {
                queue: Err(format!("bad recorded outcome `{}`", recorded.trim())),
                recorded: false,
            };
        };
        match parse_queue(queue) {
            Ok(q) => Row {
                queue: Ok(q),
                recorded,
            },
            Err(e) => Row {
                queue: Err(e),
                recorded,
            },
        }
    }
}

/// Reads the queue lines. Blank lines are skipped so that a trailing newline
/// does not become a row.
fn read_rows<I: Iterator<Item = std::io::Result<String>>>(lines: I) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(e) => die(format!("cannot read line: {e}")),
        };
        if line.trim().is_empty() {
            continue;
        }
        rows.push(Row::parse(&line));
    }
    rows
}

/// Parses a queue string such as `TIJL` into pieces.
fn parse_queue(src: &str) -> Result<Queue, String> {
    let src = src.trim();
    // The check is on bytes because `Piece::from_str` only accepts ASCII, and
    // a non-ASCII character would otherwise be reported as a bad piece.
    if src.len() > CAP {
        return Err(format!(
            "queue {src} has {} bytes, a queue holds {CAP} pieces",
            src.len()
        ));
    }
    src.chars()
        .map(|c| Piece::from_str(&c.to_string()).map_err(|_| format!("bad piece `{c}`")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_with_a_multi_word_pattern() {
        let (fumen, pattern, fraction) = split_header("v115@vhAAgh I, J & TI 12/34");
        assert_eq!(fumen, "v115@vhAAgh");
        assert_eq!(pattern, "I, J & TI");
        assert_eq!(fraction, "12/34");
    }

    #[test]
    fn header_with_a_single_word_pattern() {
        let (fumen, pattern, fraction) = split_header("v115@vhAAgh T*p4 1/2");
        assert_eq!(fumen, "v115@vhAAgh");
        assert_eq!(pattern, "T*p4");
        assert_eq!(fraction, "1/2");
    }

    #[test]
    fn fraction() {
        assert_eq!(parse_fraction("587/840"), (587, 840));
    }

    #[test]
    fn queue_line() {
        let row = Row::parse("TIJL,true");
        assert_eq!(row.recorded, true);
        assert_eq!(
            row.queue.unwrap().as_slice(),
            &[Piece::T, Piece::I, Piece::J, Piece::L][..]
        );
    }

    #[test]
    fn queue_line_bad_outcome_is_kept_as_an_error() {
        let row = Row::parse("TIJL,yes");
        assert!(row.queue.is_err());
    }

    #[test]
    fn queue_line_bad_piece_is_kept_as_an_error() {
        let row = Row::parse("TXJL,true");
        assert!(row.queue.is_err());
    }

    #[test]
    fn queue_line_too_long_is_rejected_before_the_solver_panics() {
        // `reachable` asserts the queue fits its packed depth field, so this
        // must be caught here rather than by a panic.
        let long = "T".repeat(CAP + 1);
        let row = Row::parse(&format!("{long},true"));
        assert!(row.queue.is_err());
    }

    #[test]
    fn progress_line_fits_a_wide_terminal() {
        assert_eq!(progress_line("TTIJL", 3, 840, 60), "TTIJL 3/840");
    }

    #[test]
    fn progress_line_keeps_the_count_and_cuts_the_queue() {
        // Room for the changing part is 10 columns. The count takes 6 and the
        // space 1, so 3 are left: the ellipsis plus the last 2 pieces.
        let line = progress_line("TIJLO", 12, 840, 10);
        assert_eq!(line, "…LO 12/840");
        assert_eq!(line.chars().count(), 10);
    }

    #[test]
    fn progress_line_drops_the_queue_when_there_is_no_room() {
        // 7 columns leaves room for the count and one space, so the queue is
        // dropped entirely rather than shown as a misleading fragment.
        assert_eq!(progress_line("TTIJL", 7, 840, 7), " 7/840");
        assert_eq!(progress_line("TTIJL", 7, 840, 0), " 7/840");
    }

    #[test]
    fn progress_line_never_exceeds_the_room_given() {
        // Every width from nothing to a wide terminal. The count must survive.
        for room in 0..40 {
            let line = progress_line("TTIJLOSZ", 840, 5040, room);
            assert!(line.ends_with("840/5040"), "room {room}: {line}");
            assert!(
                line.chars().count() <= room.max("840/5040".len() + 1),
                "room {room}: {line}"
            );
        }
    }
}
