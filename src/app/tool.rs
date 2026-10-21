//! Toolbar tools, their shortcuts and labels.

use crate::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    Hand,
    #[default]
    Select,
    Rectangle,
    Diamond,
    Ellipse,
    Arrow,
    Line,
    Draw,
    Text,
    Eraser,
    Frame,
    Sticky,
    AutoShape,
    Laser,
    Fill,
    Lasso,
    Terminal,
    Portal,
    Ide,
}

impl Tool {
    /// The main toolbar row; drawing tools sit in a second row opened by its last button.
    pub(crate) const TOOLBAR: [Tool; 7] = [Tool::Hand, Tool::Select, Tool::Terminal, Tool::Portal, Tool::Ide, Tool::Sticky, Tool::Text];

    pub(crate) const DRAWING: [Tool; 7] =
        [Tool::Rectangle, Tool::Diamond, Tool::Ellipse, Tool::Arrow, Tool::Line, Tool::Draw, Tool::Eraser];

    /// Tools in the drawing row's "⋮ Mais ferramentas" menu.
    pub(crate) const EXTRA: [Tool; 5] = [Tool::Frame, Tool::AutoShape, Tool::Laser, Tool::Fill, Tool::Lasso];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tool::Hand => "Mão (ferramenta de rolagem)",
            Tool::Select => "Seleção",
            Tool::Rectangle => "Retângulo",
            Tool::Diamond => "Losango",
            Tool::Ellipse => "Elipse",
            Tool::Arrow => "Flecha",
            Tool::Line => "Linha",
            Tool::Draw => "Desenhar",
            Tool::Text => "Texto",
            Tool::Eraser => "Borracha",
            Tool::Frame => "Ferramenta de quadro",
            Tool::Sticky => "Nota adesiva",
            Tool::AutoShape => "Desenhar para criar forma",
            Tool::Laser => "Ponteiro de laser",
            Tool::Fill => "Preenchimento",
            Tool::Lasso => "Seleção de laço",
            Tool::Terminal => "Terminal",
            Tool::Portal => "Portal (navegador)",
            Tool::Ide => "IDE (pasta de projeto)",
        }
    }

    /// Letter shortcut first, numeric second (HelpDialog.tsx).
    pub(crate) fn keys(self) -> &'static [Key] {
        match self {
            Tool::Hand => &[Key::H],
            Tool::Select => &[Key::V, Key::Num1],
            Tool::Rectangle => &[Key::R, Key::Num2],
            Tool::Diamond => &[Key::D, Key::Num3],
            Tool::Ellipse => &[Key::O, Key::Num4],
            Tool::Arrow => &[Key::A, Key::Num5],
            Tool::Line => &[Key::L, Key::Num6],
            Tool::Draw => &[Key::P, Key::Num7],
            Tool::Text => &[Key::T, Key::Num8],
            Tool::Eraser => &[Key::E, Key::Num0],
            Tool::Frame => &[Key::F],
            Tool::Sticky => &[Key::N],
            Tool::AutoShape => &[], // Shift+X, handled with the shortcuts
            Tool::Laser => &[Key::K],
            Tool::Fill => &[Key::B],
            Tool::Lasso | Tool::Terminal | Tool::Portal | Tool::Ide => &[],
        }
    }

    /// The shortcut on its toolbar button and in the "Mais ferramentas" menu.
    pub(crate) fn badge(self) -> &'static str {
        match self {
            Tool::Hand => "",
            Tool::AutoShape => "Shift+X",
            tool => tool.keys().first().map_or("", |k| k.name()),
        }
    }
}
