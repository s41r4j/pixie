//! From what the model says to running text, and from running text to keystrokes.
//!
//! The model writes in capitals, without punctuation, one piece (up to a pause) at a time.
//! While they speak Pixie shows it in lower case; the tidy-up when they stop adds the
//! punctuation and capitals.

/// One piece as people would type it: lower case, single spaces, "i" as "I".
pub fn tidy(piece: &str) -> String {
    let lower = piece.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    for word in lower.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        match word {
            "i" => out.push('I'),
            w if w.starts_with("i'") => {
                out.push('I');
                out.push_str(&w[1..]);
            }
            w => out.push_str(w),
        }
    }
    out
}

/// Everything heard in one dictation: the pieces that ended in a pause, and the one in progress.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Transcript {
    done: String,
    piece: String,
}

impl Transcript {
    /// The model's newest reading of the piece in progress.
    pub fn hear(&mut self, piece: &str) {
        self.piece = tidy(piece);
    }

    /// A pause: the piece in progress is finished and the next one starts empty.
    pub fn pause(&mut self) {
        if !self.piece.is_empty() {
            if !self.done.is_empty() {
                self.done.push(' ');
            }
            self.done.push_str(&self.piece);
            self.piece.clear();
        }
    }

    pub fn text(&self) -> String {
        match (self.done.is_empty(), self.piece.is_empty()) {
            (_, true) => self.done.clone(),
            (true, false) => self.piece.clone(),
            (false, false) => format!("{} {}", self.done, self.piece),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty() && self.piece.is_empty()
    }
}

/// The fewest keystrokes that turn `typed` into `want`: how many characters to delete from the
/// end, then what to type. Words that haven't changed are never touched, so text appears to
/// grow as they speak instead of being rewritten.
pub fn keystrokes<'a>(typed: &str, want: &'a str) -> (usize, &'a str) {
    let same: usize = typed.chars().zip(want.chars()).take_while(|(a, b)| a == b).count();
    let cut = want.char_indices().nth(same).map(|(i, _)| i).unwrap_or(want.len());
    (typed.chars().count() - same, &want[cut..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_read_like_typing() {
        assert_eq!(tidy("HELLO   WORLD"), "hello world");
        assert_eq!(tidy(" I THINK I'M RIGHT AND I'LL SAY IT "), "I think I'm right and I'll say it");
        assert_eq!(tidy("IT IS"), "it is");
        assert_eq!(tidy(""), "");
    }

    #[test]
    fn a_dictation_grows_piece_by_piece() {
        let mut t = Transcript::default();
        assert!(t.is_empty());
        t.hear("SEND THE");
        assert_eq!(t.text(), "send the");
        t.hear("SEND THE REPORT");
        t.pause();
        t.pause(); // a second pause with nothing new changes nothing
        assert_eq!(t.text(), "send the report");
        t.hear("BY FRIDAY");
        assert_eq!(t.text(), "send the report by friday");
        t.hear("");
        assert_eq!(t.text(), "send the report");
    }

    #[test]
    fn only_the_changed_end_is_retyped() {
        // Growing: nothing deleted.
        assert_eq!(keystrokes("send the", "send the report"), (0, " report"));
        // The model changed its mind about the last word.
        assert_eq!(keystrokes("send the reporter", "send the report by"), (2, " by"));
        // Nothing new.
        assert_eq!(keystrokes("same", "same"), (0, ""));
        // From nothing, and back to nothing.
        assert_eq!(keystrokes("", "hi"), (0, "hi"));
        assert_eq!(keystrokes("hi", ""), (2, ""));
        // Characters, not bytes.
        assert_eq!(keystrokes("café au", "café ol"), (2, "ol"));
    }
}
