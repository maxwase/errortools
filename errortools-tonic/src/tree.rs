//! Wire encoding of the `errortools-tree-bin` trailer.
//!
//! A small length-prefixed format, written by hand so the crate needs no
//! serialization dependency:
//!
//! ```text
//! tree   := version nodes          version is the byte 0x01
//! nodes  := count node*            count nodes follow
//! node   := 0x00 string chain      a leaf: context, then its error chain
//!         | 0x01 string nodes      a group: label, then its children
//! chain  := count string*          at least one message, outermost first
//! string := count byte*            UTF-8
//! count  := unsigned LEB128
//! ```

use errortools::{Detached, Node, sources};

use crate::{DetachedErrors, Detail};

/// Format version, the first byte of the trailer.
const VERSION: u8 = 1;
/// Node tag of a leaf.
const LEAF: u8 = 0;
/// Node tag of a group.
const GROUP: u8 = 1;
/// Deepest group nesting that is encoded or decoded. Decoding recurses per
/// level, so the bound keeps hostile input from exhausting the stack.
const MAX_DEPTH: usize = 32;

/// Encodes `errors`, or returns `None` when it nests deeper than
/// [`MAX_DEPTH`]. With [`Detail::TopLevel`] only each leaf's own message is
/// written.
pub(crate) fn encode(errors: &DetachedErrors, detail: Detail) -> Option<Vec<u8>> {
    let mut out = vec![VERSION];
    encode_nodes(errors, detail, MAX_DEPTH, &mut out)?;
    Some(out)
}

fn encode_nodes(
    errors: &DetachedErrors,
    detail: Detail,
    depth_left: usize,
    out: &mut Vec<u8>,
) -> Option<()> {
    write_count(errors.len(), out);
    for node in errors {
        match node {
            Node::Leaf(leaf) => {
                out.push(LEAF);
                write_str(&leaf.context, out);
                match detail {
                    Detail::TopLevel => {
                        write_count(1, out);
                        write_str(leaf.error.message(), out);
                    }
                    Detail::Chain => {
                        write_count(sources(&leaf.error).count(), out);
                        let levels =
                            std::iter::successors(Some(&leaf.error), |level| level.cause());
                        for level in levels {
                            write_str(level.message(), out);
                        }
                    }
                }
            }
            Node::Group(group) => {
                out.push(GROUP);
                write_str(&group.context, out);
                encode_nodes(&group.errors, detail, depth_left.checked_sub(1)?, out)?;
            }
        }
    }
    Some(())
}

fn write_count(mut value: usize, out: &mut Vec<u8>) {
    loop {
        // Truncation is the point: the low seven bits go out first.
        let low = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

fn write_str(value: &str, out: &mut Vec<u8>) {
    write_count(value.len(), out);
    out.extend_from_slice(value.as_bytes());
}

/// Decodes a trailer. Returns `None` for anything that is not exactly one
/// well-formed tree: unknown version, truncated or trailing bytes, invalid
/// UTF-8, a leaf without a message, or nesting deeper than [`MAX_DEPTH`].
pub(crate) fn decode(bytes: &[u8]) -> Option<DetachedErrors> {
    let mut reader = Reader(bytes);
    if reader.byte()? != VERSION {
        return None;
    }
    let errors = reader.nodes(MAX_DEPTH)?;
    reader.0.is_empty().then_some(errors)
}

/// Rebuilds a chain from its messages, outermost first. `None` when there
/// are no messages.
pub(crate) fn detached_from_messages(
    messages: impl DoubleEndedIterator<Item = String>,
) -> Option<Detached> {
    messages.rev().fold(None, |source, message| {
        let level = Detached::new(message);
        Some(match source {
            Some(source) => level.with_source(source),
            None => level,
        })
    })
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (&first, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(first)
    }

    fn count(&mut self) -> Option<usize> {
        let mut value = 0usize;
        for shift in (0..usize::BITS).step_by(7) {
            let byte = self.byte()?;
            let bits = usize::from(byte & 0x7f);
            // Reject bits that would be shifted out of the value.
            if bits.checked_shl(shift)? >> shift != bits {
                return None;
            }
            value |= bits << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    fn string(&mut self) -> Option<String> {
        let len = self.count()?;
        let (bytes, rest) = self.0.split_at_checked(len)?;
        self.0 = rest;
        String::from_utf8(bytes.to_vec()).ok()
    }

    // The counts are never used to preallocate: every item consumes at least
    // one byte, so the input length bounds the work whatever the counts say.
    fn nodes(&mut self, depth_left: usize) -> Option<DetachedErrors> {
        let mut errors = DetachedErrors::new();
        for _ in 0..self.count()? {
            let tag = self.byte()?;
            let context = self.string()?;
            match tag {
                LEAF => {
                    let mut messages = Vec::new();
                    for _ in 0..self.count()? {
                        messages.push(self.string()?);
                    }
                    errors.push(context, detached_from_messages(messages.into_iter())?);
                }
                GROUP => errors.push_group(context, self.nodes(depth_left.checked_sub(1)?)?),
                _ => return None,
            }
        }
        Some(errors)
    }
}
