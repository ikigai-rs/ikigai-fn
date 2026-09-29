//! The proof: the tutorial's tic-tac-toe BOARD view as pure composition.
//!
//! In `ikigai-tutorial` (crates/tic-tac-toe, `view_board`, at ec622cc) the board is a
//! template whose `{{square x y}}` slots Rust code fills: it reads the winner and each
//! cell, picks one of three square templates, and fills that template's `{{x}}`, `{{y}}`
//! and `{{mark}}`. Here the same board is a set of templates and NAMES, and the only code
//! is the stub cell space the tutorial already has (an atom, a cell, a winner):
//!
//! | name | kind | how |
//! |---|---|---|
//! | `…:stored:{x}:{y}` | ATOM | the mark; `NotFound` when unplayed |
//! | `…:cell:{x}:{y}` | tiny function | the stored mark, or `-` |
//! | `…:winner` | tiny function | `X`, `O`, `draw`, or `-` |
//! | `…:template:{name}` | resource | HTML with markers |
//! | `…:view:board` | composition | `composeOver` the `board` template |
//! | `…:view:square:{x}:{y}` | composition | `conditional` on the cell `equals=-` |
//! | `…:view:square-empty:{x}:{y}` | composition | `conditional` on the winner `equals=-` |
//! | `…:view:square-{open,taken,closed}:{x}:{y}` | composition | the square templates |
//!
//! The tutorial's filler, copied below as `rust_board` over the tutorial's own templates,
//! is the reference, and the composed board must equal it BYTE FOR BYTE in every state.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use common::*;
use ikigai_core::{
    AsyncFnEndpoint, EndpointSpace, Error, Exact, FnEndpoint, Invocation, InvokeFuture, Iri,
    Kernel, Representation, Result, UriTemplate,
};

/// What an empty cell reads as, and what the winner is while nobody has won.
const EMPTY: &str = "-";
const DRAW: &str = "draw";

// ---- the stub cell space (what the tutorial already has) --------------------------

fn name_of(pattern: &str, x: i64, y: i64) -> Iri {
    iri(&pattern
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string()))
}

fn coordinate(inv: &Invocation<'_>, name: &str) -> Result<i64> {
    inv.bindings
        .get(name)
        .ok_or_else(|| Error::MissingArgument(name.to_string()))?
        .parse()
        .map_err(|_| Error::InvalidArgument {
            name: name.to_string(),
            detail: "not an integer".to_string(),
        })
}

async fn text_of(inv: &Invocation<'_>, name: &Iri) -> Result<String> {
    let repr = inv.source(name).await?;
    String::from_utf8(repr.bytes).map_err(|_| Error::Endpoint("not UTF-8".to_string()))
}

/// The cell: the stored mark, or `-` — its `NotFound` is the empty cell, and hangs the
/// answer from the stored cell's thread.
fn cell() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("cell", |inv: &Invocation<'_>| -> InvokeFuture<'_> {
        Box::pin(async move {
            let (x, y) = (coordinate(inv, "x")?, coordinate(inv, "y")?);
            let mark = match inv
                .source(&name_of("urn:example:ttt:stored:{x}:{y}", x, y))
                .await
            {
                Ok(played) => played.bytes,
                Err(Error::NotFound(_)) => EMPTY.as_bytes().to_vec(),
                Err(other) => return Err(other),
            };
            Ok(Representation::new(text_html(), mark).cacheable())
        })
    })
}

/// The eight lines of a 3×3 board.
const LINES: [[(i64, i64); 3]; 8] = [
    [(0, 0), (1, 0), (2, 0)],
    [(0, 1), (1, 1), (2, 1)],
    [(0, 2), (1, 2), (2, 2)],
    [(0, 0), (0, 1), (0, 2)],
    [(1, 0), (1, 1), (1, 2)],
    [(2, 0), (2, 1), (2, 2)],
    [(0, 0), (1, 1), (2, 2)],
    [(2, 0), (1, 1), (0, 2)],
];

/// Who has won: a mark, `draw`, or `-` while the game goes on.
fn winner() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("winner", |inv: &Invocation<'_>| -> InvokeFuture<'_> {
        Box::pin(async move {
            let mut marks = BTreeMap::new();
            for (x, y) in LINES.iter().flatten() {
                let mark = text_of(inv, &name_of("urn:example:ttt:cell:{x}:{y}", *x, *y)).await?;
                marks.insert((*x, *y), mark);
            }
            let won = LINES.iter().find_map(|line| {
                let first = &marks[&line[0]];
                (first != EMPTY && line.iter().all(|at| &marks[at] == first)).then_some(first)
            });
            let answer = match won {
                Some(mark) => mark.clone(),
                None if marks.values().all(|mark| mark != EMPTY) => DRAW.to_string(),
                None => EMPTY.to_string(),
            };
            Ok(Representation::new(text_html(), answer.into_bytes()).cacheable())
        })
    })
}

// ---- the templates -----------------------------------------------------------------

/// The tutorial's templates, verbatim (`crates/tic-tac-toe/templates/*.html`, final
/// newline dropped as the tutorial drops it), for the Rust filler.
const SLOT_TEMPLATES: &[(&str, &str)] = &[
    (
        "slots:board",
        "<div class=\"ttt-grid\" role=\"group\" aria-label=\"The board: row 0 is the top row, column 0 the left\">
<div class=\"ttt-row\">{{square 0 0}}{{square 1 0}}{{square 2 0}}</div>
<div class=\"ttt-row\">{{square 0 1}}{{square 1 1}}{{square 2 1}}</div>
<div class=\"ttt-row\">{{square 0 2}}{{square 1 2}}{{square 2 2}}</div>
</div>",
    ),
    (
        "slots:square-open",
        "<button type=\"button\" class=\"ttt-square\" id=\"ttt-square-{{x}}-{{y}}\" hx-post=\"iki/tutorial/ttt/view/play/{{x}}/{{y}}\" hx-target=\"previous .ttt-status\" aria-label=\"{{x}},{{y}}: empty, play here\"></button>",
    ),
    (
        "slots:square-taken",
        "<button type=\"button\" class=\"ttt-square\" id=\"ttt-square-{{x}}-{{y}}\" aria-disabled=\"true\" aria-label=\"{{mark}} at {{x}},{{y}}\">{{mark}}</button>",
    ),
    (
        "slots:square-closed",
        "<button type=\"button\" class=\"ttt-square\" id=\"ttt-square-{{x}}-{{y}}\" aria-disabled=\"true\" aria-label=\"{{x}},{{y}}: empty, the game is over\"></button>",
    ),
];

/// The same four pieces as compose templates, plus the two that make the choice the Rust
/// code made. `{{x}}` became `$h{{x}}`, `{{mark}}` became `$h{<the cell>}`, and
/// `{{square x y}}` became `$r{<the square's view>}` — `$r`, because a composed view is
/// already expanded and must not be expanded again.
const COMPOSE_TEMPLATES: &[(&str, &str)] = &[
    (
        "board",
        "<div class=\"ttt-grid\" role=\"group\" aria-label=\"The board: row 0 is the top row, column 0 the left\">
<div class=\"ttt-row\">$r{urn:example:ttt:view:square:0:0}$r{urn:example:ttt:view:square:1:0}$r{urn:example:ttt:view:square:2:0}</div>
<div class=\"ttt-row\">$r{urn:example:ttt:view:square:0:1}$r{urn:example:ttt:view:square:1:1}$r{urn:example:ttt:view:square:2:1}</div>
<div class=\"ttt-row\">$r{urn:example:ttt:view:square:0:2}$r{urn:example:ttt:view:square:1:2}$r{urn:example:ttt:view:square:2:2}</div>
</div>",
    ),
    (
        "square",
        "$r{urn:iki:fn:conditional?if=urn:example:ttt:cell:{x}:{y}&equals=-&then=urn:example:ttt:view:square-empty:{x}:{y}&else=urn:example:ttt:view:square-taken:{x}:{y}}",
    ),
    (
        "square-empty",
        "$r{urn:iki:fn:conditional?if=urn:example:ttt:winner&equals=-&then=urn:example:ttt:view:square-open:{x}:{y}&else=urn:example:ttt:view:square-closed:{x}:{y}}",
    ),
    (
        "square-open",
        "<button type=\"button\" class=\"ttt-square\" id=\"ttt-square-$h{{x}}-$h{{y}}\" hx-post=\"iki/tutorial/ttt/view/play/$h{{x}}/$h{{y}}\" hx-target=\"previous .ttt-status\" aria-label=\"$h{{x}},$h{{y}}: empty, play here\"></button>",
    ),
    (
        "square-taken",
        "<button type=\"button\" class=\"ttt-square\" id=\"ttt-square-$h{{x}}-$h{{y}}\" aria-disabled=\"true\" aria-label=\"$h{urn:example:ttt:cell:{x}:{y}} at $h{{x}},$h{{y}}\">$h{urn:example:ttt:cell:{x}:{y}}</button>",
    ),
    (
        "square-closed",
        "<button type=\"button\" class=\"ttt-square\" id=\"ttt-square-$h{{x}}-$h{{y}}\" aria-disabled=\"true\" aria-label=\"$h{{x}},$h{{y}}: empty, the game is over\"></button>",
    ),
];

/// `…:template:{name}`: every template, as cacheable HTML.
fn templates() -> FnEndpoint {
    FnEndpoint::new("template", |inv: &Invocation<'_>| {
        let name = inv
            .bindings
            .get("name")
            .ok_or_else(|| Error::MissingArgument("name".to_string()))?;
        let (_, body) = SLOT_TEMPLATES
            .iter()
            .chain(COMPOSE_TEMPLATES)
            .find(|(known, _)| *known == name)
            .ok_or_else(|| Error::NotFound(format!("no template `{name}`")))?;
        Ok(Representation::new(text_html(), body.as_bytes().to_vec()).cacheable())
    })
}

// ---- the reference: the tutorial's Rust filler -------------------------------------

/// A slot, `{{name}}` or `{{name 1 2}}` — the tutorial's `Slot`, `fill` and `escape`,
/// trimmed to what the board needs.
struct Slot {
    name: String,
    args: Vec<i64>,
}

fn fill(template: &str, mut value: impl FnMut(&Slot) -> Result<(bool, String)>) -> Result<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let close = after.find("}}").expect("a closed slot");
        let mut words = after[..close].split(' ');
        let slot = Slot {
            name: words.next().unwrap_or_default().to_string(),
            args: words.map(|w| w.parse().expect("an integer")).collect(),
        };
        let (is_text, filled) = value(&slot)?;
        out.push_str(&if is_text { escape(&filled) } else { filled });
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn escape(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// The tutorial's `view_board`: the board template with each `{{square x y}}` filled by
/// the square template the cell calls for.
fn rust_board() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("rust-board", |inv: &Invocation<'_>| -> InvokeFuture<'_> {
        Box::pin(async move {
            let template = |name: &str| iri(&format!("urn:example:ttt:template:slots:{name}"));
            let board = text_of(inv, &template("board")).await?;
            let over = text_of(inv, &iri("urn:example:ttt:winner")).await? != EMPTY;
            let mut squares = BTreeMap::new();
            // Collect the slots first (a fill closure cannot await).
            let mut coords = Vec::new();
            fill(&board, |slot| {
                coords.push((slot.args[0], slot.args[1]));
                Ok((false, String::new()))
            })?;
            for (x, y) in coords {
                let mark = text_of(inv, &name_of("urn:example:ttt:cell:{x}:{y}", x, y)).await?;
                let kind = match (mark != EMPTY, over) {
                    (true, _) => "square-taken",
                    (false, false) => "square-open",
                    (false, true) => "square-closed",
                };
                let square = text_of(inv, &template(kind)).await?;
                let html = fill(&square, |slot| match slot.name.as_str() {
                    "x" => Ok((true, x.to_string())),
                    "y" => Ok((true, y.to_string())),
                    "mark" => Ok((true, mark.clone())),
                    other => Err(Error::Endpoint(format!("unfilled slot {other}"))),
                })?;
                squares.insert((x, y), html);
            }
            let html = fill(&board, |slot| {
                Ok((false, squares[&(slot.args[0], slot.args[1])].clone()))
            })?;
            Ok(Representation::new(text_html(), html.into_bytes()).cacheable())
        })
    })
}

// ---- the kernel ------------------------------------------------------------------

const VIEW_BOARD: &str = "urn:example:ttt:view:board";
const RUST_BOARD: &str = "urn:example:ttt:rust:board";

fn kernel() -> Kernel {
    let at = |pattern: &str| UriTemplate::parse(pattern).expect("a valid template");
    let over =
        |name: &str| ikigai_fn::compose_over(iri(&format!("urn:example:ttt:template:{name}")));
    let mut space: EndpointSpace = ikigai_fn::space()
        .bind(
            at("urn:example:ttt:stored:{key}"),
            store(Arc::new(Mutex::new(BTreeMap::new()))),
        )
        .bind(at("urn:example:ttt:cell:{x}:{y}"), cell())
        .bind(Exact::new("urn:example:ttt:winner"), winner())
        .bind(at("urn:example:ttt:template:{name}"), templates())
        .bind(Exact::new(RUST_BOARD), rust_board())
        .bind(Exact::new(VIEW_BOARD), over("board"));
    for kind in [
        "square",
        "square-empty",
        "square-open",
        "square-taken",
        "square-closed",
    ] {
        space = space.bind(
            at(&format!("urn:example:ttt:view:{kind}:{{x}}:{{y}}")),
            over(kind),
        );
    }
    Kernel::new(Arc::new(space))
}

fn play(kernel: &Kernel, x: i64, y: i64, mark: &str) {
    sink(kernel, &format!("urn:example:ttt:stored:{x}:{y}"), mark).expect("a play");
}

/// The composed board and the Rust-filled one, the same bytes and the same type.
fn assert_same_board(kernel: &Kernel) -> String {
    let composed = issue(kernel, source_request(VIEW_BOARD, &[])).expect("the composed board");
    let filled = issue(kernel, source_request(RUST_BOARD, &[])).expect("the Rust board");
    let html = String::from_utf8(composed.bytes).expect("UTF-8");
    assert_eq!(html, String::from_utf8(filled.bytes).unwrap());
    assert_eq!(composed.repr_type, filled.repr_type);
    html
}

// ---- the proof -----------------------------------------------------------------

#[test]
fn a_fresh_board_is_the_same_bytes() {
    let kernel = kernel();
    let html = assert_same_board(&kernel);
    assert_eq!(html.matches("play here").count(), 9, "{html}");
}

#[test]
fn a_game_in_progress_is_the_same_bytes() {
    let kernel = kernel();
    play(&kernel, 1, 1, "X");
    play(&kernel, 0, 0, "O");
    let html = assert_same_board(&kernel);
    assert!(
        html.contains(r#"aria-label="X at 1,1">X</button>"#),
        "{html}"
    );
    assert_eq!(html.matches("play here").count(), 7);
}

/// Once someone has won, the empty squares close — the branch the Rust code took on
/// `over`, taken here by `conditional` on the winner.
#[test]
fn a_won_game_closes_the_empty_squares_the_same_way() {
    let kernel = kernel();
    for (x, y, mark) in [
        (0, 0, "X"),
        (0, 1, "O"),
        (1, 1, "X"),
        (0, 2, "O"),
        (2, 2, "X"),
    ] {
        play(&kernel, x, y, mark);
    }
    let html = assert_same_board(&kernel);
    assert_eq!(html.matches("the game is over").count(), 4, "{html}");
    assert!(!html.contains("play here"));
}

#[test]
fn a_draw_is_the_same_bytes() {
    let kernel = kernel();
    for (x, y, mark) in [
        (0, 0, "X"),
        (1, 0, "O"),
        (2, 0, "X"),
        (1, 1, "O"),
        (0, 1, "X"),
        (2, 1, "O"),
        (1, 2, "X"),
        (0, 2, "O"),
        (2, 2, "X"),
    ] {
        play(&kernel, x, y, mark);
    }
    assert_same_board(&kernel);
}

/// A mark is text: escaped into the HTML, and a marker inside it is not expanded — the
/// composed board terminates at the atom, exactly as the Rust filler never looked.
#[test]
fn a_hostile_mark_is_escaped_and_terminated_the_same_way() {
    let kernel = kernel();
    play(&kernel, 0, 0, r#"<b>"&'$a{urn:example:ttt:winner}"#);
    let html = assert_same_board(&kernel);
    assert!(
        html.contains(r#">&lt;b&gt;&quot;&amp;&#39;$a{urn:example:ttt:winner}</button>"#),
        "{html}"
    );
    assert!(!html.contains("<b>"));
}

/// The ETag and the golden thread. The composed board is cached, and its content
/// address is stable across reads — a hash of its type and bytes, the same two inputs
/// `ikigai-web`'s strong ETag hashes, so the ETag moves exactly when this does. A Sink on
/// one stored cell cuts it: the next read recomputes it and the ETag moves. Only what READ
/// that cell moves with it: a square taken elsewhere, which never reads the winner, is
/// still cached.
#[test]
fn a_sink_on_one_cell_recomputes_the_view_and_moves_its_etag() {
    let kernel = kernel();
    play(&kernel, 1, 1, "X");
    let board = source_request(VIEW_BOARD, &[]);
    let etag = issue(&kernel, board.clone()).unwrap().content_id();
    assert!(cached(&kernel, &board));
    assert_eq!(issue(&kernel, board.clone()).unwrap().content_id(), etag);

    let taken = source_request("urn:example:ttt:view:square:1:1", &[]);
    let open = source_request("urn:example:ttt:view:square:0:1", &[]);
    assert!(cached(&kernel, &taken) && cached(&kernel, &open));

    play(&kernel, 2, 2, "O");
    assert!(!cached(&kernel, &board), "the Sink cut the board");
    assert!(
        !cached(&kernel, &open),
        "an empty square reads the winner, which read 2,2"
    );
    assert!(cached(&kernel, &taken), "X at 1,1 read only its own cell");

    let after = issue(&kernel, board.clone()).unwrap();
    assert_ne!(after.content_id(), etag, "the ETag moved");
    assert!(String::from_utf8(after.bytes).unwrap().contains("O at 2,2"));
    assert!(
        cached(&kernel, &board),
        "and the recomputed board is cached again"
    );
    assert_same_board(&kernel);
}

/// The template is a resource too: the board hangs from it like any other name it read.
#[test]
fn the_view_depends_on_its_template() {
    let kernel = kernel();
    let board = source_request(VIEW_BOARD, &[]);
    let repr = issue(&kernel, board).unwrap();
    let threads: Vec<&str> = repr.threads().iter().map(|t| t.as_str()).collect();
    for expected in [
        "urn:example:ttt:template:board",
        "urn:example:ttt:template:square",
        "urn:example:ttt:stored:0:0",
        "urn:example:ttt:winner",
    ] {
        assert!(threads.contains(&expected), "{expected} in {threads:?}");
    }
}
