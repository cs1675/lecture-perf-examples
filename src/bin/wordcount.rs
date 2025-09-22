use std::collections::HashMap;
use std::io::Read;

fn main() {
    let filename = std::env::args()
        .skip(1)
        .next()
        .expect("want filename argument");
    println!("{:?}", filename);
    let mut f = std::fs::File::open(filename).expect("open file");
    let mut file_contents_vec = Vec::new();
    f.read_to_end(&mut file_contents_vec).expect("read file");
    let file_contents = String::from_utf8_lossy(&file_contents_vec);
    let wordcount: HashMap<&str, usize> = file_contents.split(|c: char| !c.is_alphanumeric()).fold(
        HashMap::default(),
        |mut acc, w| {
            acc.entry(w).and_modify(|c| *c += 1).or_insert(1);
            acc
        },
    );

    let mut cnts: Vec<(&str, usize)> = wordcount.into_iter().collect();
    cnts.sort_by_key(|(_, c)| *c);

    for (word, count) in cnts {
        println!("{}: {}", word, count);
    }
}
