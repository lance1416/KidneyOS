use crate::rush::cd::cd;
use crate::rush::clear::clear;
use crate::rush::env::CURR_DIR;
use crate::rush::ls::ls_config::LsConfig;
use crate::rush::ls::ls_core::list;
use crate::rush::pwd::pwd;
use crate::threading::process_functions::exit_process;
use alloc::string::ToString;
use alloc::vec::Vec;
use kidneyos_shared::eprintln;

pub(crate) fn parse_input(input: &str) {
    let mut tokens = input.split_whitespace();
    let command = tokens.next().unwrap_or("");
    let args = tokens.collect::<Vec<&str>>();

    match command {
        "cat" => {
            // print the contents of a file
        }
        "cd" => {
            // change directory
            cd(args);
        }
        "clear" => {
            // clear the screen
            clear();
        }
        "echo" => {
            // print the arguments
        }
        "exit" => {
            // What the exit system call does.
            exit_process(0);
        }
        "ls" => {
            let config = LsConfig::from_args(args);
            let curr_dir = CURR_DIR.read().to_string();
            list(curr_dir.as_ref(), config);
        }
        "pwd" => {
            // print working directory
            pwd();
        }
        _ => {
            // command not found
            eprintln!("rush: {}: command not found", command);
        }
    }
}
