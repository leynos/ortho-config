//! What a child's output says about waiting for compile admission.
//!
//! On a shared host a nested Cargo child spends most of a long run queued for a
//! compile slot, printing `waiting for a compile slot` lines. The retained
//! tail of its output begins after those lines, so a deadline failure that
//! showed only the tail could not tell a slow start from a hang (#755). The
//! scanner here counts the queue lines over the *whole* stream, however much of
//! it the tail kept, and the deadline error leads with the verdict.

/// The phrase a child prints while it waits for a compile-admission slot.
const QUEUE_LINE_MARKER: &str = "waiting for a compile slot";

/// Longest line the scanner holds while it waits for the line's end.
///
/// A line longer than this is a compiler diagnostic, not a queue line, so the
/// scanner drops its tail rather than growing without bound.
const SCAN_LINE_BYTES: usize = 4096;

/// What a child's output said about waiting for a compile slot.
///
/// Two streams of one child are combined by summing the waits and keeping the
/// deepest queue. There is no "last depth": each stream is read on its own
/// thread, so which stream's last line came later is not recorded, and a
/// combined figure would report stream priority as observation order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AdmissionQueue {
    /// Lines that said the child was waiting for a slot.
    waits: u64,
    /// The largest number of requests the child reported being ahead of it.
    longest: u64,
}

impl AdmissionQueue {
    /// Count `line` if it is a queue line.
    fn observe(&mut self, line: &[u8]) {
        let text = String::from_utf8_lossy(line);
        if !text.contains(QUEUE_LINE_MARKER) {
            return;
        }
        self.waits = self.waits.saturating_add(1);
        self.longest = self.longest.max(requests_ahead(&text));
    }

    /// Combine the evidence of two streams of the same child.
    pub(super) fn merge(self, other: Self) -> Self {
        Self {
            waits: self.waits.saturating_add(other.waits),
            longest: self.longest.max(other.longest),
        }
    }

    /// A sentence that attributes a deadline to a queue wait, or rules one out.
    pub(super) fn verdict(self) -> String {
        if self.waits == 0 {
            return "no compile-admission queue lines were seen, so the child was not \
                    waiting for a slot; a deadline here is the child's own work or a hang"
                .to_owned();
        }
        format!(
            "the child printed {} compile-admission queue lines (at most {} requests \
             ahead), so time was spent waiting for a slot, not compiling",
            self.waits, self.longest,
        )
    }
}

/// The request count in `... (N ahead)`, or zero when the line gives none.
fn requests_ahead(line: &str) -> u64 {
    let Some((before, _after)) = line.split_once("ahead") else {
        return 0;
    };
    let reversed_digits: String = before
        .trim_end()
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();
    let digits: String = reversed_digits.chars().rev().collect();
    digits.parse().unwrap_or(0)
}

/// Feeds a stream's chunks to an [`AdmissionQueue`], one whole line at a time.
#[derive(Default)]
pub(super) struct QueueScanner {
    /// The unfinished last line, held until its end arrives.
    partial_line: Vec<u8>,
    /// What the complete lines said.
    queue: AdmissionQueue,
}

impl QueueScanner {
    /// What the lines fed so far said, counting an unfinished last line.
    ///
    /// A child killed at its deadline may have been cut off mid-line, and its
    /// last queue line is the one that matters most, so the unterminated line
    /// is read as well. It is not stored: when its newline arrives it is
    /// counted once, as a complete line.
    pub(super) fn queue(&self) -> AdmissionQueue {
        let mut queue = self.queue;
        queue.observe(&self.partial_line);
        queue
    }

    /// Scan `chunk`.
    ///
    /// A line can straddle two reads, so the unfinished end of each chunk is
    /// held and joined to the start of the next.
    pub(super) fn feed(&mut self, chunk: &[u8]) {
        let mut pieces = chunk.split(|byte| *byte == b'\n');
        let Some(first) = pieces.next() else {
            return;
        };
        self.extend_partial(first);
        for piece in pieces {
            self.queue.observe(&self.partial_line);
            self.partial_line.clear();
            self.extend_partial(piece);
        }
    }

    /// Append to the unfinished line, up to the scanner's line bound.
    fn extend_partial(&mut self, piece: &[u8]) {
        let room = SCAN_LINE_BYTES.saturating_sub(self.partial_line.len());
        self.partial_line.extend(piece.iter().take(room).copied());
    }
}

#[cfg(test)]
mod tests {
    //! Counting queue lines, wherever the reads split.

    use super::{AdmissionQueue, QueueScanner};

    /// Two queue lines among other output, as a waiting child prints them.
    const QUEUE_OUTPUT: &str = "   Compiling serde v1.0.0\n\
        waiting for a compile slot (12 ahead)\n\
        waiting for a compile slot (3 ahead)\n\
        error: something unrelated\n";

    /// The scanner's reading of `text`, fed as one chunk.
    fn scan(text: &str) -> AdmissionQueue {
        let mut scanner = QueueScanner::default();
        scanner.feed(text.as_bytes());
        scanner.queue()
    }

    /// A queue line is counted once wherever the read boundary falls.
    #[test]
    fn queue_lines_are_counted_once_however_the_reads_split() {
        let expected = scan(QUEUE_OUTPUT);
        for split in 0..=QUEUE_OUTPUT.len() {
            let (head, rest) = QUEUE_OUTPUT.as_bytes().split_at(split);
            let mut scanner = QueueScanner::default();
            scanner.feed(head);
            scanner.feed(rest);
            assert_eq!(
                scanner.queue(),
                expected,
                "a read boundary at byte {split} changed the queue count"
            );
        }
    }

    /// Output with no queue line is reported as not queued.
    #[test]
    fn a_stream_without_queue_lines_rules_a_queue_wait_out() {
        let queue = scan("   Compiling serde v1.0.0\nerror[E0432]: unresolved import\n");
        assert_eq!(
            queue,
            AdmissionQueue::default(),
            "a stream with no queue lines must leave the admission counters empty"
        );
        assert!(
            queue.verdict().contains("not waiting for a slot"),
            "an unqueued child should be reported as such: {}",
            queue.verdict(),
        );
    }

    /// A queue wait is attributed, with its depth.
    #[test]
    fn a_queued_child_is_attributed_with_its_depth() {
        let verdict = scan(QUEUE_OUTPUT).verdict();
        assert!(
            verdict.contains("2 compile-admission queue lines")
                && verdict.contains("at most 12 requests ahead"),
            "the verdict should name the count and depth: {verdict}",
        );
    }

    /// A child cut off mid-line still has its last queue line counted.
    #[test]
    fn an_unterminated_final_queue_line_is_counted() {
        let queue = scan("   Compiling serde v1.0.0\nwaiting for a compile slot (12 ahead)");
        assert_eq!(
            queue.verdict(),
            AdmissionQueue {
                waits: 1,
                longest: 12
            }
            .verdict(),
            "a queue line with no newline yet must be counted when the evidence is read",
        );
    }

    /// An unterminated line is counted once, not again when its newline arrives.
    #[test]
    fn an_unterminated_line_is_not_counted_twice() {
        let mut scanner = QueueScanner::default();
        scanner.feed(b"waiting for a compile slot (4 ahead)");
        let before = scanner.queue();
        scanner.feed(b"\n");
        assert_eq!(
            scanner.queue(),
            before,
            "completing a line that was already read must not count it again"
        );
    }

    /// A line far longer than the scan bound is dropped, not accumulated.
    #[test]
    fn an_overlong_line_is_not_held_without_bound() {
        let mut scanner = QueueScanner::default();
        scanner.feed(&vec![b'x'; 1_000_000]);
        scanner.feed(b"\nwaiting for a compile slot (1 ahead)\n");
        assert_eq!(
            scanner.queue(),
            AdmissionQueue {
                waits: 1,
                longest: 1
            },
            "the queue line after an overlong line should still be counted",
        );
    }

    /// Two streams of one child combine by summing waits and keeping the deepest queue.
    #[test]
    fn merging_streams_sums_waits_and_keeps_the_deepest_queue() {
        let merged = scan("waiting for a compile slot (40 ahead)\n")
            .merge(scan("waiting for a compile slot (5 ahead)\n"));
        assert_eq!(
            merged,
            AdmissionQueue {
                waits: 2,
                longest: 40
            },
            "merging should sum the waits and keep the deepest queue",
        );
    }
}
