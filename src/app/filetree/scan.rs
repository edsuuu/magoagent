//! The syntax problems of the project's files that are not open in the editor, found on a worker thread when the tree starts
//! (and again for a file when its tab closes), so the tree reddens them as the reference IDE does.

use crate::app::filetree::{code::files_under, problems::check, syntax};
use crate::editor::highlight;
use crate::editor::lsp::Diag;
use crate::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};

/// The most files and the biggest file the background scan reads.
const FILES: usize = 4000;
const BIGGEST: u64 = 256 << 10;

type Answer = (PathBuf, Vec<Diag>);

pub(crate) struct Scanned {
    found: HashMap<PathBuf, Vec<Diag>>,
    channel: (Sender<Answer>, Receiver<Answer>),
    started: bool,
}

impl Scanned {
    pub(crate) fn new() -> Self {
        Scanned { found: HashMap::new(), channel: channel(), started: false }
    }

    /// Takes what the workers found; true when something changed.
    pub(crate) fn poll(&mut self) -> bool {
        let answers: Vec<Answer> = self.channel.1.try_iter().collect();
        let changed = !answers.is_empty();

        self.found.extend(answers);
        changed
    }

    /// Starts the first scan of `root` (once; never under test, which reads files itself).
    pub(crate) fn start(&mut self, root: &Path, ctx: &Context) {
        if self.started || cfg!(test) || !root.is_dir() {
            return;
        }

        self.started = true;
        let (root, sender, ctx) = (root.to_path_buf(), self.channel.0.clone(), ctx.clone());

        std::thread::spawn(move || {
            let files =
                files_under(&root, FILES, |path| syntax::handles(highlight::detect(path).id) && std::fs::metadata(path).is_ok_and(|m| m.len() <= BIGGEST));

            read_all(files, &sender, &ctx);
        });
    }

    /// Reads `path` again from the disk (its tab closed, or it was saved).
    pub(crate) fn refresh(&mut self, path: &Path, ctx: &Context) {
        if cfg!(test) || !syntax::handles(highlight::detect(path).id) {
            return;
        }

        let (files, sender, ctx) = (vec![path.to_path_buf()], self.channel.0.clone(), ctx.clone());

        std::thread::spawn(move || read_all(files, &sender, &ctx));
    }

    pub(crate) fn get(&self, path: &Path) -> &[Diag] {
        self.found.get(path).map_or(&[], Vec::as_slice)
    }

    /// The files with some problem.
    pub(crate) fn files(&self) -> impl Iterator<Item = &PathBuf> {
        self.found.iter().filter(|(_, diags)| !diags.is_empty()).map(|(path, _)| path)
    }
}

fn read_all(files: Vec<PathBuf>, sender: &Sender<Answer>, ctx: &Context) {
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let diags = check(&path, highlight::detect(&path), &text.replace("\r\n", "\n"));

        if sender.send((path, diags)).is_err() {
            return;
        }

        ctx.request_repaint();
    }
}
