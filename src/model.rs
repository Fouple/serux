use std::path::PathBuf;
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

pub type DocFreq = HashMap<String, usize>;
pub type TermFreq = HashMap<String, usize>;
pub type TermFreqPerDoc = HashMap<PathBuf, TermFreq>;

#[derive(Default, Deserialize, Serialize)]
pub struct Model {
    pub tfpd: TermFreqPerDoc,
    pub df: DocFreq,
}

/// Term frequency
///  tf(t,d), is the relative frequency of term t within document d
pub fn compute_tf(t: &str, d: &TermFreq) -> f32 {
    // a:  f(t,d) is the raw count of a term in a document
    let a = d.get(t).cloned().unwrap_or(0) as f32;
    // b: sum of  the raw count of a term in a document
    let b = d.iter().map(|(_, f)| *f).sum::<usize>() as f32;
    a / b
}

/// Inverse document frequency
/// idf(t,D) is a measure of how much information the word provides
pub fn compute_idf(t: &str, n: usize, df: &DocFreq) -> f32 {
    // total number of documents in the corpus
    let n = n as f32;
    // number of documents where the term t appears
    // tip: If the term is not in the corpus, this will lead to a division-by-zero
    let m = df.get(t).cloned().unwrap_or(1) as f32;
    // Narrow down the range of values
    (n / m).ln()
}

#[derive(Debug)]
pub struct Lexer<'a> {
    content: &'a [char],
}

impl<'a> Lexer<'a> {
    pub fn new(content: &'a [char]) -> Self {
        Self { content }
    }

    // Trim leading whitespace
    fn trim_left(&mut self) {
        while !self.content.is_empty() && self.content[0].is_whitespace() {
            self.content = &self.content[1..];
        }
    }

    // Remove n characters from the beginning of the content
    fn chop(&mut self, n: usize) -> &'a [char] {
        let token = &self.content[0..n];
        self.content = &self.content[n..];
        token
    }

    fn chop_while<P>(&mut self, mut predicate: P) -> &'a [char] where P: FnMut(&char) -> bool {
        let mut n = 0;
        while n < self.content.len() && predicate(&self.content[n]) {
            n += 1;
        }
        self.chop(n)
    }

    pub fn next_token(&mut self) -> Option<String> {
        self.trim_left();
        if self.content.len() == 0 {
            return None;
        }

        if self.content[0].is_numeric() {
            return Some(self.chop_while(|x| x.is_numeric()).iter().collect());
        }

        if self.content[0].is_alphabetic() {
            return Some(self.chop_while(|x| x.is_alphanumeric()).iter().map(|x| x.to_ascii_uppercase()).collect());
        }

        return Some(self.chop(1).iter().collect());
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

pub fn search_query<'a>(model: &'a Model, query: &'a [char]) -> Vec<(&'a PathBuf, f32)> {
    // let tokens = Lexer::new(&query).collect::<Vec<_>>();
    // let mut results = Vec::<(&PathBuf, f32)>::new();
    // for (path, tf_table) in tf_index {
    //     let mut rank = 0f32;
    //     for token in &tokens {
    //         rank += tf(&token, &tf_table) * idf(&token, &tf_index);
    //     }
    //     results.push((path, rank));
    // }
    let tokens = Lexer::new(&query).collect::<HashSet<String>>();
    let mut results: Vec::<(&PathBuf, f32)> = model.tfpd.iter().map(|(path, tf_table)| {
        let mut rank = 0f32;
        for token in &tokens {
            rank += compute_tf(&token, &tf_table) * compute_idf(&token, model.tfpd.len(),&model.df);
        }
        (path, rank)
    }).filter(|(_, rank)| *rank > 0f32).collect();
    results.sort_by(|(_, rank1), (_, rank2)| rank2.partial_cmp(rank1).unwrap());
    results
}