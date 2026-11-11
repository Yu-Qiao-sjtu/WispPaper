//! The text of a PDF's pages, read with hayro (the interpreter the preview
//! draws with) from where each glyph stands: glyphs become lines, lines
//! become paragraphs, and a blank line is put wherever the page shows a
//! break that plain text would lose: a larger gap, a change of size or
//! weight (a heading), a first-line indent, a jump to the next column. A
//! bold title that runs into its paragraph gets a line of its own, and the
//! raised citation numbers of a sentence are left out. `structure` reads
//! paragraphs and headings off that text.

use hayro_interpret::font::{Glyph, GlyphRun};
use hayro_interpret::hayro_cmap::BfString;
use hayro_interpret::hayro_syntax::{DecryptionError, LoadPdfError, Pdf};
use hayro_interpret::kurbo::{BezPath, Point, Rect};
use hayro_interpret::{BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps, InterpreterCache, InterpreterSettings, SoftMask, TransformExt, interpret_page};

/// One glyph as the page draws it, in points from the top left.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub text: String,
    /// Where it starts and ends along the line.
    pub x0: f32,
    pub x1: f32,
    /// Its baseline.
    pub y: f32,
    /// The size of its font.
    pub size: f32,
    pub bold: bool,
}

#[derive(Default)]
struct Marks {
    marks: Vec<Mark>,
    /// Glyphs the font gives no character for.
    unreadable: usize,
}

impl<'a> Device<'a> for Marks {
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, _: &DrawMode) {
        // A run is set in one font: its weight is read once.
        let bold = run.glyphs().iter().find_map(|glyph| match &**glyph {
            Glyph::Outline(outline) => outline.font_data().map(|font| font.weight.is_some_and(|w| w >= 600) || font.postscript_name.is_some_and(|name| ["bold", "black", "heavy", "semibold", "demi"].iter().any(|word| name.to_lowercase().contains(word)))),
            Glyph::Type3(_) => None,
        });
        for glyph in run.glyphs() {
            let text = match glyph.as_unicode() {
                // A ligature is spelled out, so a word reads the same however it was set.
                Some(BfString::Char(c)) => match c {
                    'ﬀ' => "ff".into(),
                    'ﬁ' => "fi".into(),
                    'ﬂ' => "fl".into(),
                    'ﬃ' => "ffi".into(),
                    'ﬄ' => "ffl".into(),
                    // The non-breaking hyphen some journals break words with is a hyphen to a reader.
                    '\u{2011}' => "-".into(),
                    c => c.to_string(),
                },
                Some(BfString::String(text)) => text,
                None => {
                    self.unreadable += 1;
                    continue;
                }
            };
            let advance = match &**glyph {
                Glyph::Outline(outline) => outline.advance_width(),
                Glyph::Type3(type3) => type3.advance_width(),
            };
            // Glyph space is a thousandth of the font size.
            let place = props.transform * glyph.transform();
            let (origin, right, up) = (place * Point::new(0., 0.), place * Point::new(advance.unwrap_or(500.) as f64, 0.), place * Point::new(0., 1000.));
            let size = (up - origin).hypot() as f32;
            // Text set sideways or upside down is a figure's, or a notice in the margin.
            if (right.y - origin.y).abs() > (right.x - origin.x).abs() || right.x < origin.x || !(1. ..400.).contains(&size) { continue }
            self.marks.push(Mark { text, x0: origin.x as f32, x1: right.x as f32, y: origin.y as f32, size, bold: bold.unwrap_or(false) });
        }
    }
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// The text of every page of a PDF. An error when the file cannot be
/// parsed, or when too many of its glyphs name no character for the text
/// to be trusted.
pub fn pages(data: Vec<u8>) -> Result<Vec<String>, String> {
    let pdf = Pdf::new(data).map_err(|err| match err {
        LoadPdfError::Decryption(DecryptionError::PasswordProtected) => crate::fulltext::LOCKED.to_string(),
        LoadPdfError::Decryption(other) => format!("这个 PDF 加了密，用的方式这里解不开（{other:?}）。"),
        LoadPdfError::Invalid => "无法解析这个 PDF 的结构。".to_string(),
    })?;
    let cache = InterpreterCache::new();
    let mut out = vec![];
    let (mut read, mut unreadable) = (0, 0);
    for page in pdf.pages().iter() {
        let (width, height) = page.render_dimensions();
        let marks = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut marks = Marks::default();
            let mut context = Context::new(page.initial_transform(true).to_kurbo(), Rect::new(0., 0., width as f64, height as f64), &cache, page.xref(), InterpreterSettings::default());
            interpret_page(page, &mut context, &mut marks);
            marks
        }))
        .unwrap_or_default();
        read += marks.marks.len();
        unreadable += marks.unreadable;
        out.push(page_text(&marks.marks));
    }
    if unreadable > read { return Err(crate::fulltext::NO_ENCODING.into()) }
    Ok(out)
}

/// A line of a page: its text, where it stands and how it is set.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    text: String,
    x0: f32,
    x1: f32,
    y: f32,
    size: f32,
    bold: bool,
    /// When the line opens in bold and goes on in regular: the bytes of the bold opening.
    bold_opening: usize,
}

/// Whether a raised run of small glyphs is a citation: numbers, commas and dashes only.
fn is_citation(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit()) && text.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '-' | '–' | ' ' | '\u{a0}'))
}

/// The marks of one line made into its text.
fn line_of(marks: &[Mark]) -> Option<Line> {
    // The size most of the line is set in, and the baseline of that text.
    let mut sizes: Vec<(f32, usize)> = vec![];
    for mark in marks {
        match sizes.iter_mut().find(|(size, _)| (*size - mark.size).abs() < 0.3) {
            Some((_, count)) => *count += mark.text.len(),
            None => sizes.push((mark.size, mark.text.len())),
        }
    }
    let size = sizes.iter().max_by_key(|(_, count)| *count)?.0;
    let body: Vec<&Mark> = marks.iter().filter(|m| (m.size - size).abs() < 0.3).collect();
    let y = body.iter().map(|m| m.y).sum::<f32>() / body.len() as f32;
    let raised = |m: &Mark| m.size < size * 0.82 && m.y < y - size * 0.12;

    let mut text = String::new();
    let (mut bold_bytes, mut plain_bytes, mut bold_opening, mut opening_over) = (0, 0, 0, false);
    // Where the glyph before ended, a left-out citation included.
    let mut last: Option<f32> = None;
    let mut at = 0;
    while at < marks.len() {
        let mark = &marks[at];
        if raised(mark) {
            let end = (at..marks.len()).find(|i| !raised(&marks[*i])).unwrap_or(marks.len());
            let run: String = marks[at..end].iter().map(|m| m.text.as_str()).collect();
            let before = text.trim_end().chars().last();
            // After a word or a clause, raised numbers cite; after a digit they are a power.
            if is_citation(&run) && before.is_none_or(|c| !c.is_ascii_digit()) {
                last = Some(marks[end - 1].x1);
                at = end;
                continue;
            }
        }
        if let Some(last) = last {
            let gap = mark.x0 - last;
            if (gap > size * 0.11 || gap < -size) && !text.ends_with(' ') && !mark.text.starts_with(' ') { text.push(' ') }
        }
        if !mark.text.trim().is_empty() {
            if mark.bold { bold_bytes += mark.text.len() } else { plain_bytes += mark.text.len() }
            if !mark.bold { opening_over = true }
        }
        text.push_str(&mark.text);
        if !opening_over { bold_opening = text.len() }
        last = Some(mark.x1);
        at += 1;
    }
    let trimmed = text.trim_end();
    if trimmed.trim().is_empty() { return None }
    let bold_opening = if opening_over && bold_opening < trimmed.len() { bold_opening } else { 0 };
    Some(Line { text: trimmed.to_string(), x0: marks.iter().map(|m| m.x0).fold(f32::MAX, f32::min), x1: marks.iter().map(|m| m.x1).fold(f32::MIN, f32::max), y, size, bold: bold_bytes > plain_bytes * 3, bold_opening })
}

/// The marks of a page, in the order they are drawn, as its lines.
fn lines(marks: &[Mark]) -> Vec<Line> {
    let mut out = vec![];
    let mut start = 0;
    for at in 1..=marks.len() {
        let ends = at == marks.len() || {
            let (last, next) = (&marks[at - 1], &marks[at]);
            let size = last.size.max(next.size);
            // Another baseline, or back to the left: a new line.
            (next.y - last.y).abs() > size * 0.55 || next.x0 < last.x1 - size * 2.
        };
        if ends {
            out.extend(line_of(&marks[start..at]));
            start = at;
        }
    }
    out
}

/// The text of a page from its marks: one line of text per line of the
/// page, a blank line wherever the page shows a break.
pub fn page_text(marks: &[Mark]) -> String {
    let lines = lines(marks);
    let mut out = String::new();
    let mut opens = true;
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            let last = &lines[index - 1];
            let size = last.size.max(line.size);
            let breaks = line.y < last.y - size * 0.5
                || line.y - last.y > size * 1.75
                || (line.size - last.size).abs() > 0.6
                || line.bold != last.bold
                || (line.x0 > last.x0 + size * 0.9 && line.x0 < last.x0 + size * 4. && last.x1 - last.x0 > size * 12.)
                || (line.x0 - last.x0).abs() > size * 5.
                // A short line that opens a paragraph and stops without a mark, before a full line: a title set like the text.
                || (opens && last.x1 - last.x0 < (line.x1 - line.x0) * 0.75 && !last.text.ends_with(['.', ',', ';', ':', '-', '‑']) && line.text.chars().next().is_some_and(|c| c.is_uppercase()) && last.text.chars().next().is_some_and(|c| c.is_uppercase() || c.is_ascii_digit()));
            opens = breaks;
            out.push_str(if breaks { "\n\n" } else { "\n" });
        }
        // A bold title run into its paragraph: `Plant materials. Seeds were sown…`.
        let opening = &line.text[..line.bold_opening];
        if opens && opening.trim().chars().count() >= 8 && opening.split_whitespace().count() >= 2 && line.text.len() - line.bold_opening >= 20 {
            out.push_str(opening.trim());
            out.push_str("\n\n");
            out.push_str(line.text[line.bold_opening..].trim());
        } else {
            out.push_str(&line.text);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line of text set glyph by glyph from `x`, each half its size wide.
    fn set(text: &str, x: f32, y: f32, size: f32, bold: bool) -> Vec<Mark> {
        text.chars().enumerate().map(|(i, c)| Mark { text: c.to_string(), x0: x + i as f32 * size * 0.5, x1: x + (i + 1) as f32 * size * 0.5, y, size, bold }).collect()
    }

    #[test]
    fn lines_and_breaks() {
        let mut marks = vec![];
        marks.extend(set("Results", 50., 100., 12., true));
        marks.extend(set("Roots adapt to compacted soil in many ways", 50., 116., 10., false));
        // A raised citation after the word, then the sentence goes on.
        marks.extend(set("1,2", 260., 112., 6., false));
        marks.extend(set(". They grow.", 269., 116., 10., false));
        marks.extend(set("and they thicken as the soil gets harder", 50., 128., 10., false));
        // An indented first line opens the next paragraph.
        marks.extend(set("A second paragraph starts here.", 62., 140., 10., false));
        // The next column starts higher up.
        marks.extend(set("The other column.", 320., 100., 10., false));
        assert_eq!(page_text(&marks), "Results\n\nRoots adapt to compacted soil in many ways. They grow.\nand they thicken as the soil gets harder\n\nA second paragraph starts here.\n\nThe other column.");
    }

    #[test]
    fn spaces_powers_and_run_in_titles() {
        // Words set apart by a gap and no space glyph; a power stays, unlike a citation.
        let mut marks = set("about", 50., 100., 10., false);
        marks.extend(set("10", 80., 100., 10., false));
        marks.extend(set("5", 90., 96., 6., false));
        marks.extend(set("cells", 98., 100., 10., false));
        assert_eq!(page_text(&marks), "about 105 cells");
        let mut marks = set("Plant materials.", 50., 100., 10., true);
        marks.extend(set(" Seeds were sown on soil.", 130., 100., 10., false));
        assert_eq!(page_text(&marks), "Plant materials.\n\nSeeds were sown on soil.");
        assert_eq!(page_text(&[]), "");
    }
}
