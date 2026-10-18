//! Linexin Installer: the interface a person sees, and — started by polkit
//! with `--worker` — the root worker that writes the disk, or — with
//! `--measure` — the read-only measurement of its partitions.
//!
//! **Linexin Installer** to a person, `lxb-installer` to the machine: the
//! name in the title bar and on the desktop entry is the translated one; the
//! executable, the window class, the package and the polkit action are all
//! `lxb-installer`.

use lxb_app::lxb_toolkit::input::Action;
use lxb_app::App;
use lxb_installer::i18n;
use lxb_installer::ui::{glyphs, State};

/// The machine's name for this program: the executable, the window class and
/// the id its desktop entry is filed under.
const APP_ID: &str = "lxb-installer";

const HELP: &str = "\
Linexin Installer — puts Linexin on this computer.

Usage: lxb-installer [OPTIONS]

With no options it opens the installer. Nothing is written to any disk
until Install is pressed on the last screen and the question after it is
answered, and then only from a Linexin live system.

  --demo            a made-up computer with made-up disks; Install only
                    rehearses, and nothing on this computer is touched
  --shot FILE       write one settled frame of the demo to a PNG and stop
  --width N         how wide that frame is (default 1600)
  --height N        how tall that frame is (default 900)
  --scene NAME      which screen the frame shows: welcome, language,
                    language-search, timezone, timezone-continue,
                    timezone-places, timezone-search,
                    timezone-search-nothing, keyboard, keyboard-layouts,
                    keyboard-continue, keyboard-none, keyboard-search,
                    keyboard-search-nothing, target,
                    target-erase, target-free, target-nothing,
                    target-whole, target-kinds, target-options,
                    target-storage, target-refused,
                    target-partitions-in-use, partitions,
                    partitions-nvme, partitions-msdos, partitions-tiny,
                    partitions-blocked, partitions-menu,
                    partitions-mount, partitions-table,
                    partitions-planned, partitions-options,
                    partitions-pending, partitions-missing,
                    partitions-new, partitions-new-held,
                    partitions-new-typed, partitions-resize,
                    partitions-label, partitions-other,
                    partitions-new-planned,
                    partitions-continue-refused,
                    partitions-portrait-slid,
                    partitions-unmountable, partitions-measured,
                    partitions-hibernated,
                    partitions-hibernated-other, partitions-reading,
                    desktop, desktop-options, desktop-software,
                    desktop-loaders-plan, account, account-empty,
                    account-administrator, review, confirm,
                    review-partitions, confirm-partitions,
                    confirm-plan-pages, confirm-plan-last,
                    installing, installing-details,
                    installing-shrink, done, done-shrink, stopped,
                    stopped-details, stopped-shrink,
                    stopped-hibernated, stopped-needs-check,
                    stopped-windows-in-use, stopped-windows-check,
                    stopped-windows-unread, stopped-bitlocker,
                    stopped-windows-shrunk or board
  --pad             draw that frame as with a controller in hand; with
                    --demo, open the window as with one in hand, which
                    the pointer does not put down and the first key does
  --shells-board    draw that frame as under LineXinBar with a controller in
                    hand and LineXinBar's on-screen keyboard typing into
                    the field the light is in: the screen's room ends over
                    where it stands, the keyboard itself (LineXinBar's) is
                    not drawn
  --pointer X,Y     draw that frame with the pointer resting at X,Y (in
                    the frame's pixels), what it rests on lit, and a ring
                    where it is; under a menu or a question, which take
                    the pointer, nothing of the page is lit and no ring
                    is drawn
  --shot-at SECONDS the frame that long into a change rather than a settled
                    one, the frames a sixtieth of a second apart: with
                    --from, after the page goes from that scene to --scene
                    (Back where --scene comes before it); without, after
                    --scene is set up, so a menu or a question it raises is
                    caught opening
  --from NAME       the scene a --shot-at change starts from, drawn
                    settled first
  --press NAME      the press a --shot-at change is, answered after
                    --scene is drawn settled: accept, back, up, down,
                    left, right or options (Back closing a column, Down
                    rolling one, Options raising its menu)
  --then NAME       a further press, --then-at SECONDS after the first
                    (Continue and then Back at once); the two may be
                    given again, in pairs, for a third press and more
  --greeting N      with --scene welcome, start on the Nth of Welcome's
                    twenty greetings (1 to 20) rather than the language's
                    own
  --search WORDS    with language-search, timezone-search or
                    keyboard-search, those words typed into the search in
                    place of the scene's own
  --low-end         draw as on low-end hardware, the window's clock held
                    still: a page changes at once, nothing of the
                    installer's own moves, and every light, the question's
                    and the menus' too, stands on what is chosen
  --list-disks      print the disks as the installer reads them, as JSON,
                    and stop; nothing is changed
  --version         print the version
  --help            print this message

  D-pad or arrows   move            South or Enter   choose
  East or Escape    back            North or menu    options

--worker and --measure are the installer's own, each started by polkit as
root and accepted only alone. --worker reads the installation from its
standard input and carries it out. --measure only reads: how full the
partitions of the disks it is asked about are and whether they could be
made smaller, and it changes nothing on any disk. Neither is for running by
hand, and anywhere but on the Linexin live medium each refuses before doing
anything.";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = root_half(&arguments) {
        std::process::exit(code);
    }
    if let Err(error) = run(&arguments) {
        eprintln!("lxb-installer: {error}");
        std::process::exit(1);
    }
}

/// The two roads polkit opens, each taken only when its flag is the whole
/// command line: the worker (`--worker`, the installation's action) and the
/// read-only measurement (`--measure`, an action of its own). Nothing else is
/// parsed for either, so no flag given beside one can change what the root
/// half does; beside anything at all, each is `run`'s to refuse. The exit
/// status of the one taken, or `None`. In main.rs's own tests, which stand
/// outside the library's fence, each stops before it starts.
fn root_half(arguments: &[String]) -> Option<i32> {
    if arguments == ["--worker"] {
        not_in_a_test("--worker would start the root worker");
        return Some(lxb_installer::worker::main());
    }
    if arguments == ["--measure"] {
        not_in_a_test("--measure would read this machine's disks");
        return Some(lxb_installer::worker::measure::main());
    }
    None
}

/// The flags the help names that are followed by a value.
const TAKE_A_VALUE: [&str; 12] = [
    "--shot",
    "--width",
    "--height",
    "--scene",
    "--pointer",
    "--shot-at",
    "--from",
    "--press",
    "--then",
    "--then-at",
    "--greeting",
    "--search",
];

/// The flags the help names that stand alone. `--worker` and `--measure`
/// are here only so that `run` refuses them in words: alone, each is
/// `root_half`'s before `run` is reached.
const STAND_ALONE: [&str; 10] = [
    "--demo",
    "--pad",
    "--shells-board",
    "--low-end",
    "--help",
    "-h",
    "--version",
    "--list-disks",
    "--worker",
    "--measure",
];

fn run(arguments: &[String]) -> Result<(), String> {
    let has = |flag: &str| arguments.iter().any(|one| one == flag);
    let named = |flag: &str| -> Option<&String> {
        arguments
            .iter()
            .position(|one| one == flag)
            .and_then(|at| arguments.get(at + 1))
    };
    // Every value given to a flag that may be given more than once, in order.
    let every = |flag: &str| -> Vec<Option<&String>> {
        arguments
            .iter()
            .enumerate()
            .filter(|(_, one)| *one == flag)
            .map(|(at, _)| arguments.get(at + 1))
            .collect()
    };
    // Anything not in the help is refused before anything is read or opened.
    // A mistyped flag falling through to the installer itself would, on a
    // developer's machine, be the real installer reading its real disks: only
    // no arguments at all opens it.
    let mut rest = arguments.iter();
    while let Some(one) = rest.next() {
        if TAKE_A_VALUE.contains(&one.as_str()) {
            rest.next();
        } else if !STAND_ALONE.contains(&one.as_str()) {
            return Err(format!(
                "{one} is not an option of this program (see --help)"
            ));
        }
    }
    if has("--worker") {
        return Err("--worker is started by polkit, alone, and never by hand".into());
    }
    if has("--measure") {
        return Err("--measure is started by polkit, alone, and never by hand".into());
    }
    if has("--help") || has("-h") {
        println!("{HELP}");
        return Ok(());
    }
    if has("--version") {
        println!("lxb-installer {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if has("--list-disks") {
        not_in_a_test("--list-disks reads this machine's disks");
        let disks = lxb_installer::model::disks::discover()?;
        let json = serde_json::to_string_pretty(&disks).map_err(|error| error.to_string())?;
        println!("{json}");
        return Ok(());
    }
    if has("--shot") {
        let path = named("--shot").ok_or("--shot needs a file to write")?;
        let number = |flag: &str, default: u32| -> Result<u32, String> {
            named(flag).map_or(Ok(default), |value| {
                value
                    .parse()
                    .map_err(|_| format!("{flag} wants a number, not {value}"))
            })
        };
        let width = number("--width", 1600)?;
        let height = number("--height", 900)?;
        if !(320..=7680).contains(&width) || !(320..=4320).contains(&height) {
            return Err("the frame must be between 320x320 and 7680x4320".into());
        }
        let scene = named("--scene").map_or("welcome", String::as_str);
        let pointer = match named("--pointer") {
            None if has("--pointer") => return Err("--pointer wants X,Y".into()),
            None => None,
            Some(value) => Some(point(value)?),
        };
        let at = match named("--shot-at") {
            None if has("--shot-at") => return Err("--shot-at wants a number of seconds".into()),
            None => None,
            Some(value) => match value.parse::<f32>() {
                Ok(seconds) if (0.0..=10.0).contains(&seconds) => Some(seconds),
                _ => return Err(format!("--shot-at wants seconds from 0 to 10, not {value}")),
            },
        };
        let from = match (named("--from"), at) {
            (None, _) if has("--from") => return Err("--from wants a scene".into()),
            (Some(_), None) => return Err("--from names where a --shot-at change starts".into()),
            (from, _) => from.map(String::as_str),
        };
        let press = match (named("--press"), at) {
            (None, _) if has("--press") => return Err("--press wants a press".into()),
            (Some(_), None) => return Err("--press names what a --shot-at change is".into()),
            (Some(_), Some(_)) if from.is_some() => {
                return Err("a change is --from a scene or a --press, not both".into())
            }
            (Some(name), _) => Some(pressed(name)?),
            (None, _) => None,
        };
        let (names, times) = (every("--then"), every("--then-at"));
        if !names.is_empty() && press.is_none() {
            return Err("--then is a press after a --press".into());
        }
        if names.len() != times.len() {
            return Err("--then and --then-at go together".into());
        }
        let mut then = Vec::new();
        for (name, value) in names.into_iter().zip(times) {
            let name = name.ok_or("--then wants a press")?;
            let value = value.ok_or("--then-at wants a number of seconds")?;
            match value.parse::<f32>() {
                Ok(seconds) if (0.0..=10.0).contains(&seconds) => {
                    then.push((pressed(name)?, seconds))
                }
                _ => return Err(format!("--then-at wants seconds from 0 to 10, not {value}")),
            }
        }
        let greeting = match named("--greeting") {
            None if has("--greeting") => return Err("--greeting wants a number".into()),
            None => None,
            Some(value) => Some(
                value
                    .parse::<usize>()
                    .map_err(|_| format!("--greeting wants 1 to 20, not {value}"))?,
            ),
        };
        let search = match named("--search") {
            None if has("--search") => return Err("--search wants words".into()),
            search => search.map(String::as_str),
        };
        let picture = Picture {
            scene,
            greeting,
            search,
            pad: has("--pad"),
            shells_board: has("--shells-board"),
            pointer,
            low_end: has("--low-end"),
            moving: at.map(|seconds| Moving {
                seconds,
                from,
                press,
                then,
            }),
        };
        return shot(path, width, height, picture);
    }

    // Past the flags only the installer itself is left — the real one
    // without --demo, reading this machine. main.rs's own tests expect every
    // argument they give to be refused before this, and stand outside the
    // library's fence (`worker::run::fence`), so a broken refusal stops here.
    not_in_a_test("the installer would open");
    let mut state = if has("--demo") {
        let mut state = State::demo(false);
        state.start_in(&session_locale());
        state
    } else {
        State::real()
    };
    if has("--demo") && has("--pad") {
        state.pretend_a_pad_until_a_key();
    }
    // Under LineXinBar a pad types with the shell's own board, which rises
    // over a field that takes text; anywhere else with the installer's.
    state.type_with_the_shells_board(lxb_installer::ui::board::under_linexinbar_here());
    glyphs::added(App::new(APP_ID, i18n::text("app-name")).driven().plain())
        .run(move |page| state.frame(page))
}

/// The locale the session speaks, read as the toolkit reads the language
/// its words are said in (`LC_ALL`, then `LC_MESSAGES`, then `LANG`), so
/// the made-up machine's language and its places agree with its words.
fn session_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

/// What `--press NAME` presses.
fn pressed(name: &str) -> Result<Action, String> {
    Ok(match name {
        "accept" => Action::Accept,
        "back" => Action::Back,
        "up" => Action::Up,
        "down" => Action::Down,
        "left" => Action::Left,
        "right" => Action::Right,
        "options" => Action::Menu,
        other => {
            return Err(format!(
                "--press wants accept, back, up, down, left, right or options, not {other}"
            ))
        }
    })
}

/// `X,Y` as a place in a picture.
fn point(value: &str) -> Result<[f32; 2], String> {
    let wrong = || format!("--pointer wants X,Y, not {value}");
    let (x, y) = value.split_once(',').ok_or_else(wrong)?;
    let x: f32 = x.trim().parse().map_err(|_| wrong())?;
    let y: f32 = y.trim().parse().map_err(|_| wrong())?;
    Ok([x, y])
}

/// What a picture shows.
struct Picture<'a> {
    scene: &'a str,
    /// Welcome's greeting to start on, from 1.
    greeting: Option<usize>,
    /// What is typed into the scene's search.
    search: Option<&'a str>,
    pad: bool,
    /// Under LineXinBar with its board typing into the field the light is
    /// in, summoned once the first frame has said which field that is.
    shells_board: bool,
    pointer: Option<[f32; 2]>,
    low_end: bool,
    /// A frame part of the way through a change rather than a settled one.
    moving: Option<Moving<'a>>,
}

/// A frame `seconds` into a change: from the scene `from` to the picture's
/// own, or a `press` on the picture's scene (and `then` others, each so many
/// seconds after it), or — with neither — from the moment the picture's
/// scene is set up.
struct Moving<'a> {
    seconds: f32,
    from: Option<&'a str>,
    press: Option<Action>,
    then: Vec<(Action, f32)>,
}

/// The instant every picture is taken at, so the wallpaper behind it is the
/// same on every one.
const SHOT_AT: f32 = 6.0;

/// The instant a picture of low-end hardware is held at: the one the window
/// holds its wallpaper at there.
const LOW_END_AT: f32 = lxb_app::lxb_toolkit::settings::STILL_WALLPAPER_AT;

/// A picture of one screen of the demo, with no display at all: the same
/// frame and the same renderer as the window, which is what makes it worth
/// looking at. `App::shot` draws twice at one instant; between the two
/// everything is put where it is going. A frame part of the way through a
/// change is `App::shot_after`'s instead: frame after frame, a sixtieth of a
/// second apart, the change made between the first (settled) and the second
/// as a press would make it.
/// In main.rs's own tests, which stand outside the library's fence: stops
/// the test before `what` happens. Every test here gives arguments `run`
/// must refuse, so reaching one of these is the refusal broken, and the
/// test fails rather than reading the machine or opening a window. Nothing
/// in a release build.
fn not_in_a_test(what: &str) {
    #[cfg(test)]
    panic!("test build: {what}; run() should have refused these arguments");
    #[cfg(not(test))]
    let _ = what;
}

fn shot(path: &str, width: u32, height: u32, picture: Picture<'_>) -> Result<(), String> {
    let mut state = State::demo(true);
    // In the session's language, as a person who speaks it would have
    // chosen it: its catalog (the toolkit reads that from the session on
    // its own), the places named in it and its guesses.
    state.start_in(&session_locale());
    if picture.pad {
        state.pretend_a_pad();
    }
    if picture.low_end {
        state.pretend_low_end();
    }
    let from = picture.moving.as_ref().and_then(|moving| moving.from);
    let press = picture.moving.as_ref().and_then(|moving| moving.press);
    let then = picture
        .moving
        .as_ref()
        .map_or_else(Vec::new, |moving| moving.then.clone());
    state.stage(from.unwrap_or(picture.scene))?;
    if let Some(nth) = picture.greeting {
        state.greet_with(nth)?;
    }
    if let Some(words) = picture.search.filter(|_| from.is_none()) {
        state.search_for(words)?;
    }
    if let Some(at) = picture.pointer {
        // What the pointer rests on is read off the first frame, so that
        // frame is drawn settled already — the board risen, not rising
        // under the pointer.
        state.pretend_a_pointer(at);
        state.settle();
    }
    // Everything a picture can refuse (a scene, a greeting, a search the
    // made-up machine has not got) is refused above, on the demo alone.
    not_in_a_test("--shot would open the renderer");
    let app = glyphs::added(App::new(APP_ID, i18n::text("app-name")).driven().plain());
    // Drawn as the window draws on low-end hardware: every frame at the one
    // instant the wallpaper is held at, so nothing the toolkit times by the
    // frame moves, and its lights stand on what is chosen.
    let (app, at) = if picture.low_end {
        (app.low_end(), LOW_END_AT)
    } else {
        (app, SHOT_AT)
    };
    // The change is made between the first frame and the second, as a press
    // between two frames is, and the second already carries a sixtieth of a
    // second of it: the frame `seconds` into it is that many sixtieths after
    // the first. With none, it is the scene as it was before the change —
    // settled, as `--shot` takes it, which the first frame on its own is not
    // (a column opened by the scene is still opening on it).
    let frames = picture
        .moving
        .as_ref()
        .map(|moving| (moving.seconds * 60.0).round() as u32)
        .filter(|frames| *frames > 0 || (from.is_none() && press.is_none()));
    let shells_board = picture.shells_board;
    let Some(frames) = frames else {
        let mut drawn = 0;
        if shells_board {
            // The shell's board is summoned on the field the first frame
            // said the light is in, and what it takes from the room is put
            // there on the second; a third, settled again, shows what the
            // smaller room scrolled into view.
            return app.shot_after(path, width, height, at, 2, move |page| {
                state.frame(page);
                drawn += 1;
                if drawn == 1 {
                    state.pretend_the_shells_board();
                }
                state.settle();
            });
        }
        return app.shot(path, width, height, at, move |page| {
            state.frame(page);
            drawn += 1;
            if drawn == 1 {
                state.settle();
            }
        });
    };
    // What a resting pointer lights is read off the first frame, after it is
    // drawn, so with a pointer the change waits for a second settled frame,
    // which shows it lit: the page a press leaves is the page as the person
    // saw it, the pointer's light on it.
    let start = if picture.pointer.is_some() && (from.is_some() || press.is_some()) {
        2
    } else {
        1
    };
    // A further press comes so many frames after the first, at least one.
    let then: Vec<(Action, u32)> = then
        .into_iter()
        .map(|(press, seconds)| {
            let after = ((seconds * 60.0).round() as u32).max(1);
            (press, start + after)
        })
        .collect();
    let scene = picture.scene.to_string();
    let mut drawn = 0;
    let mut trouble = None;
    let frames = frames + (start - 1);
    let taken = app.shot_after(path, width, height, at, frames, |page| {
        if drawn == start && from.is_some() {
            if let Err(error) = state.stage(&scene) {
                trouble = Some(error);
            }
        }
        let first = press.filter(|_| drawn == start);
        let later = then
            .iter()
            .filter(|(_, at)| *at == drawn)
            .map(|(press, _)| *press);
        for press in first.into_iter().chain(later) {
            // Handed to the window as a key is, so it goes where a key's
            // press goes: into the toolkit's question or menu where one
            // stands over the page, which answers them itself, and otherwise
            // to the page's own presses, Accept dipping what the light is on.
            page.act(press);
        }
        state.frame(page);
        drawn += 1;
        if drawn == 1 && shells_board {
            state.pretend_the_shells_board();
        }
        if drawn <= start {
            state.settle();
        }
    });
    trouble.map_or(taken, Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anything_the_help_does_not_name_is_refused_before_the_installer_opens() {
        let refused = |arguments: &[&str]| {
            let arguments: Vec<String> = arguments.iter().map(|one| one.to_string()).collect();
            run(&arguments).is_err()
        };
        assert!(refused(&["--list"]));
        assert!(refused(&["--demo", "--dmeo"]));
        assert!(refused(&["install"]));
        assert!(refused(&["--shot", "x.png", "--scene", "review", "extra"]));
        // A value is the flag's, whatever it looks like: the help would have
        // been printed for "--help" standing alone.
        assert!(refused(&["--search", "--help", "--bogus"]));
    }

    #[test]
    #[should_panic(expected = "test build: --measure would read this machine's disks")]
    fn measure_is_matched_alone() {
        // `--measure` and nothing else is the measurement's road: in a test
        // it stops before the measurement would start.
        let _ = root_half(&["--measure".to_string()]);
    }

    #[test]
    #[should_panic(expected = "test build: --worker would start the root worker")]
    fn the_worker_is_matched_alone() {
        let _ = root_half(&["--worker".to_string()]);
    }

    #[test]
    fn measure_is_refused_beside_any_flag() {
        let strings = |arguments: &[&str]| -> Vec<String> {
            arguments.iter().map(|one| one.to_string()).collect()
        };
        let mut lines: Vec<Vec<&str>> = Vec::new();
        for flag in STAND_ALONE {
            lines.push(vec!["--measure", flag]);
            lines.push(vec![flag, "--measure"]);
        }
        for flag in TAKE_A_VALUE {
            lines.push(vec!["--measure", flag, "1"]);
            lines.push(vec![flag, "1", "--measure"]);
            // As a flag's value it is refused too.
            lines.push(vec![flag, "--measure"]);
        }
        lines.push(vec!["--measure", "--measure"]);
        lines.push(vec!["--measure", "/dev/sda"]);
        lines.push(vec!["--measure="]);
        lines.push(vec!["--Measure"]);
        for line in lines {
            let arguments = strings(&line);
            // Not the root half's road, so nothing is started...
            assert_eq!(root_half(&arguments), None, "{line:?}");
            // ...and run refuses it before anything is read or opened.
            assert!(run(&arguments).is_err(), "{line:?}");
        }
        assert!(HELP.contains("--measure"));
    }

    #[test]
    #[should_panic(expected = "test build: the installer would open")]
    fn a_refusal_that_broke_stops_the_test_before_the_installer_opens() {
        // No arguments at all is the one call that opens the installer: in a
        // test it stops first.
        let _ = run(&[]);
    }

    #[test]
    fn a_picture_of_a_change_says_when_and_from_where() {
        let refused = |arguments: &[&str]| {
            let arguments: Vec<String> = arguments.iter().map(|one| one.to_string()).collect();
            run(&arguments).is_err()
        };
        assert!(
            refused(&["--shot", "x.png", "--from", "welcome"]),
            "--from alone"
        );
        assert!(
            refused(&["--shot", "x.png", "--shot-at", "11"]),
            "past ten seconds"
        );
        assert!(
            refused(&["--shot", "x.png", "--shot-at", "-1"]),
            "before the change"
        );
        assert!(refused(&["--shot", "x.png", "--shot-at"]), "no seconds");
        assert!(
            refused(&["--shot", "x.png", "--press", "back"]),
            "--press alone"
        );
        let both = [
            "--shot",
            "x.png",
            "--shot-at",
            "0.1",
            "--from",
            "welcome",
            "--press",
            "back",
        ];
        assert!(refused(&both), "--from and --press together");
        assert!(
            refused(&["--shot", "x.png", "--shot-at", "0.1", "--press", "jump"]),
            "no such press"
        );
        assert!(
            refused(&["--shot", "x.png", "--shot-at", "0.1", "--then", "back"]),
            "--then without a --press"
        );
        let then_alone = [
            "--shot",
            "x.png",
            "--shot-at",
            "0.1",
            "--press",
            "accept",
            "--then",
            "back",
        ];
        assert!(refused(&then_alone), "--then with no --then-at");
        let unpaired = [
            "--shot",
            "x.png",
            "--shot-at",
            "0.5",
            "--press",
            "options",
            "--then",
            "down",
            "--then-at",
            "0.1",
            "--then",
            "accept",
        ];
        assert!(refused(&unpaired), "a third press with no --then-at");
        assert_eq!(pressed("options"), Ok(Action::Menu));
        assert!(
            refused(&["--shot", "x.png", "--greeting", "21"]),
            "a greeting past the twenty"
        );
        assert!(refused(&["--shot", "x.png", "--greeting"]), "no number");
        assert!(
            refused(&["--shot", "x.png", "--search", "pol"]),
            "--search on a scene with no search"
        );
        for flag in [
            "--shot-at",
            "--from",
            "--press",
            "--then",
            "--low-end",
            "--shells-board",
            "--greeting",
            "--search",
        ] {
            assert!(HELP.contains(flag), "--help does not name {flag}");
        }
    }

    #[test]
    fn the_help_names_every_scene() {
        for scene in lxb_installer::ui::SCENES {
            let named = HELP
                .split(|c: char| c.is_whitespace() || c == ',')
                .any(|word| word == scene);
            assert!(named, "--help does not name the scene {scene}");
        }
    }
}
