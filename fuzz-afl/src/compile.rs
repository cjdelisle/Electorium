use std::{io::{self, BufRead, Write}, collections::HashMap};

mod names;

fn main() {
    let names = names::build();
    let mut next_num = 255_u8;
    let mut number_by_name = HashMap::new();
    let mut get_num = |name: &str| {
        if let Some(num) = number_by_name.get(name) {
            *num
        } else {
            let mut num = None;
            for (i, n) in names.iter().enumerate() {
                if *n == name {
                    num = Some(i as u8);
                }
            }
            let num = if let Some(num) = num {
                num
            } else {
                next_num -= 1;
                next_num + 1
            };
            number_by_name.insert(name.to_string(), num);
            num
        }
    };

    let mut stdout = io::stdout().lock();
    let stdin = io::stdin().lock();
    for line in stdin.lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.starts_with("#") || line == "" {
            continue;
        }
        let mut name = None;
        let mut votes = 0_u8;
        for (i, word) in line.split(' ').enumerate() {
            match i {
                0 => {
                    name = Some(word.to_owned());
                }
                1 => {
                    votes = word.parse().unwrap();
                }
                2 => {
                    let who = get_num(&name.clone().unwrap());
                    let vf = get_num(word);
                    let write = [
                        who, vf, votes,
                    ];
                    stdout.write_all(&write).unwrap();
                }
                _ => {
                    panic!("Unexpected number of words on line: {line}");
                }
            }
        }
    }
}