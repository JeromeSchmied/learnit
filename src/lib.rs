//! # Library for vocabulary learning, used in `crablit`.
use crate::enums::{Msg, SPACER};
use colored::Colorize;
use rustyline::DefaultEditor;
use std::{
    error::Error,
    fmt::Debug,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::exit,
};

/// Module for learning Deck of Cards
pub mod cards;
/// Module for parsing cli arguments
pub mod config;
/// enums
pub mod enums;
/// Module for saving state: progress
pub mod state;
// /// Module for learning Deck of Verbs
// pub mod verbs;

// re-exports
pub use cards::Card;
pub use enums::{Lok, Mode};
// pub use verbs::Verb;

// enum Kard {
//     Adjektiv(String),
//     Nomen(String),
//     Verb {
//         inf: String,
//         dri: String,
//         pra: String,
//         per: String,
//     },
//     Wendungen(String),
// }

/// Initializing deck of either `cards`, or `verbs`
///
/// # Errors
///
/// - can't read `path`
/// - can't deserialize properly
pub fn init(path: &PathBuf, delim: char) -> Result<Vec<Card>, Box<dyn Error>> {
    // contents of file with vocab data
    let contents = fs::read_to_string(path)?;
    // results vector
    let mut r = Vec::new();
    // iterating over the lines of file to store them in a vector
    for line in contents.lines() {
        // if is comment or empty
        if line.trim().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        r.push(Card::deser(line, delim)?);
    }
    eprintln!("File succesfully read.");
    // println!("content: {:?}", r);
    Ok(r)
}

/// Start learning the vector, return the remainders: ones not guessed correctly
///
/// # Errors
///
/// - `rustyline` can't create instance
pub fn question(v: &mut [Card], conf: &config::Config) -> Result<(), Box<dyn Error>> {
    // let mut printer = String::new();
    let len = v.iter().filter(|item| item.lok() != Lok::Done).count();
    println!("\n\nYou have {len} words to learn, let's start!\n\n");
    let mut rl = DefaultEditor::new()?;

    let mut i = 0;
    let mut prev_valid_i: i32;
    while i < v.len() {
        let item = &mut v[i];

        if item.lok() == Lok::Done {
            i += 1;
            continue;
        }
        prev_valid_i = i as i32 - 1;
        // display prompt
        let last_hr = rl.history().iter().last();
        // eprintln!("last history element: {:?}", last_hr);
        let msg = format!(
            "{}{SPACER}> ",
            if last_hr.is_some_and(|he| {
                he.starts_with(":h") || he == ":typo" || he == ":n" || he == ":num" || he == ":togo"
            }) {
                "".to_string()
            } else {
                format!("{}\n", item.question())
            }
        );

        let guess = rl.readline(&msg)?;
        rl.add_history_entry(&guess)?;
        let guess = guess.trim();

        // is command
        if guess.starts_with(':') {
            match guess {
                ":q" | ":quit" | ":exit" => {
                    println!("{}", Msg::Exit.val());
                    exit(0);
                }

                ":h" | ":help" | ":hint" => {
                    println!("{}", item.hint());
                }

                ":w" | ":write" | ":save" => {
                    state::save_prog(v, conf)?;
                }

                ":wq" => {
                    state::save_prog(v, conf)?;
                    println!("{}", Msg::Exit.val());
                    exit(0);
                }

                ":typo" => {
                    // ask to type again before correcting?
                    if i > 0 {
                        if let Some(skipping) = v.get(prev_valid_i as usize) {
                            println!("{}", Msg::Typo(skipping.ser(" = ")).val());
                            v[prev_valid_i as usize].incr();
                        } else {
                            println!("{}", Msg::Typo("None".to_string()).val());
                        }
                    } else {
                        println!("{}", Msg::Typo("None".to_string()).val());
                    }
                    // rl.readline(&msg)?;
                }

                ":skip" => {
                    println!("{}\n\n", item.skip());
                    i += 1;
                    continue;
                }

                ":revise" => {
                    println!("{}", Msg::Revise.val());
                    break;
                }

                ":f" | ":flash" => {
                    println!("{}\n\n\n", item.flashcard());
                    item.incr();
                    i += 1;
                }

                // incorrect, not accurate
                ":n" | ":num" | ":togo" => {
                    println!("{}", &Msg::Togo(len, (prev_valid_i + 1).try_into()?).val());
                }

                uc => {
                    println!("{} {}\n", "Unknown command:".red(), uc);
                }
            }
        } else if guess == item.correct() {
            println!("{}\n", Msg::Knew.val());
            item.incr();
            i += 1;
        } else {
            println!("{}", item.wrong());
            item.decr();
            i += 1;
        }
    }
    Ok(())
}

/// Starting program execution according to mode
///
/// # Errors
///
/// - `init()`
/// - `question()`
/// - `state::rm()`
/// - `verbs::deser_to_card()`
pub fn run(conf: &config::Config) -> Result<(), Box<dyn Error>> {
    match conf.mode() {
        Mode::Cards => {
            let mut v = init(&conf.file_path(), conf.delim())?;
            if conf.card_swap() {
                println!("swapping terms and definitions of each card");
                swap_cards(&mut v);
            }
            if conf.ask_both() {
                println!("swapping terms and definitions of some cards");
                randomly_swap_cards(&mut v);
            }

            while v.iter().filter(|item| item.lok() == Lok::Done).count() < v.len() {
                if !conf.no_shuffle() {
                    eprintln!("shuffling");
                    fastrand::shuffle(&mut v);
                }
                question(&mut v, conf)?;
            }
            println!("Gone through everything you wanted, great job!");
            state::rm_prog(&conf.file_path_orig())?;

            Ok(())
        }
        Mode::VerbsToCards => {
            let v = init(&conf.file_path(), conf.delim())?;
            let data = cards::deser_verbs_to_cards(&v, conf)?;

            let pb = PathBuf::from(&conf.file_path_orig());
            let outf_name = format!("{}_as_cards.csv", pb.file_stem().unwrap().to_str().unwrap());
            println!(
                "\n\nConverting verbs to cards, from file: {:?} to file: {}",
                conf.file_path_orig(),
                outf_name.bright_blue()
            );
            let mut out_f = File::create(outf_name)?;

            writeln!(out_f, "# [crablit]")?;
            writeln!(out_f, "# mode = \"cards\"")?;
            writeln!(out_f, "# delim = \'{}\'\n\n", conf.delim())?;
            writeln!(out_f, "{data}")?;

            println!("Converting from verbs to cards done");

            Ok(())
        }
    }
}

/// Show hint from the string got
/// # usage
/// ```
/// use crablit::hint;
///
/// let dunno_want_hint = "This is a very-very hard-to-guess sentence.";
///
/// assert_eq!("This is a very-very h______________________", hint(dunno_want_hint));
///
/// let easy = "012345";
///
/// assert_eq!("012___", hint(easy));
/// ```
pub fn hint(s: &str) -> String {
    let n = s.chars().count() / 2;
    [
        s.chars().take(n).collect::<String>(),
        s.chars().skip(n).map(|_| '_').collect(),
    ]
    .concat()
}

/// Swap definition and term of deck(vector) of cards
///
/// # usage
/// ```
/// use crablit::Card;
///
/// let mut deck = vec![Card::new("term1", "def1", None), Card::new("term2", "def2", None), Card::new("term3", "def3", None)];
///
/// crablit::swap_cards(&mut deck);
/// ```
pub fn swap_cards(cards: &mut [cards::Card]) {
    cards.iter_mut().for_each(cards::Card::swap);
}

/// Randomly swap definition and term of deck(vector) of cards
///
/// # usage
/// ```
/// use crablit::Card;
///
/// let mut deck = vec![Card::new("term1", "def1", None), Card::new("term2", "def2", None), Card::new("term3", "def3", None)];
///
/// crablit::randomly_swap_cards(&mut deck);
/// ```
pub fn randomly_swap_cards(cards: &mut [cards::Card]) {
    for card in cards.iter_mut() {
        if fastrand::bool() {
            card.swap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn incorrect_mode() {
        let _ = Mode::from("mode");
    }
    #[test]
    fn correct_mode_cards() {
        assert_eq!(Mode::Cards, Mode::from("cards"));
    }
    #[test]
    fn mode_new_conv() {
        let mode = "verbs2cards";
        assert_eq!(Mode::VerbsToCards, Mode::from(mode));
        let mode = "convert";
        assert_eq!(Mode::VerbsToCards, Mode::from(mode));
    }

    #[test]
    fn hint_not_odd() {
        let get_hint = String::from("1234");
        assert_eq!("12__", hint(&get_hint));
    }
    #[test]
    fn hint_odd() {
        let get_hint = String::from("12345");
        assert_eq!("12___", hint(&get_hint));
    }
    #[test]
    fn hint_non_ascii() {
        let get_hint = String::from("aáéűúőóüöíä|Ä");
        assert_eq!("aáéűúő_______", hint(&get_hint));
    }

    #[test]
    fn swap_cards_works() {
        let mut cards = vec![Card::new("term", "definition", None)];

        swap_cards(&mut cards);
        assert_eq!(cards, vec![Card::new("definition", "term", None)]);
    }

    // init()
    // verbs::conv()
}
