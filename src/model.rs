use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use std::result::Result;

pub trait Model {
    fn search_query(&self, query: &[char]) -> Result<Vec<(PathBuf, f32)>, ()>;
    fn add_document(&mut self, path: PathBuf, content: &[char]) -> Result<(), ()>;
}

pub struct SqliteModel {
    connection: sqlite::Connection,
}

impl SqliteModel {
    fn execute(&self, statement: &str) -> Result<(), ()> {
        self.connection.execute(statement).map_err(|err| {
            eprintln!("ERROR: could not execute query {statement}: {err}");
        })
    }

    pub fn begin(&self) -> Result<(), ()> {
        self.connection.execute("BEGIN;").map_err(log_and_ignore)
    }

    pub fn commit(&self) -> Result<(), ()> {
        self.connection.execute("COMMIT;").map_err(log_and_ignore)
    }

    pub fn open(path: &Path) -> Result<Self, ()> {
        let connection = sqlite::open(path).map_err(|err| {
            eprintln!("ERROR: could not open sqlite database {path}: {err}", path = path.display());
        })?;
        let this = Self { connection };

        // The total number of terms for a document
        this.execute("
            CREATE TABLE IF NOT EXISTS Documents (
                id INTEGER NOT NULL PRIMARY KEY,    -- 文档ID
                path TEXT,                          -- 文档路径
                term_count INTEGER,                 -- 本文档单词数量
                UNIQUE(path)                        -- 路径唯一
            );
        ")?;

        // The term frequency of a document
        this.execute("
            CREATE TABLE IF NOT EXISTS TermFreq (
                term TEXT,              -- 单词
                doc_id INTEGER,         -- 文档ID
                freq INTEGER,           -- 单词在本文档的频率
                UNIQUE(term, doc_id),   -- (单词, 文档ID)唯一
                FOREIGN KEY(doc_id) REFERENCES Documents(id)
            );
       ")?;

        // Term frequency for all documents
        this.execute("
            CREATE TABLE IF NOT EXISTS DocFreq (
                term TEXT,              -- 单词
                freq INTEGER,           -- 频率
                UNIQUE(term)
            );
        ")?;

        Ok(this)
    }
}

fn log_and_ignore(err: impl std::error::Error) {
    eprintln!("ERROR: {err}");
}

impl Model for SqliteModel {
    fn search_query(&self, query: &[char]) -> Result<Vec<(PathBuf, f32)>, ()> {
        todo!()
    }

    fn add_document(&mut self, path: PathBuf, content: &[char]) -> Result<(), ()> {
        let terms = Lexer::new(content).collect::<Vec<_>>();

        let doc_id = {
            let query = "INSERT INTO Documents (path, term_count) VALUES (:path, :count)";
            let log_err = |err| {
                eprintln!("ERROR: Could not execute query {query}: {err}");
            };
            let mut stmt = self.connection.prepare(query).map_err(log_err)?;
            stmt.bind_iter::<_,(_,sqlite::Value)>([
                (":path", path.to_str().unwrap()),
                (":count", (terms.len() as i64).into()),
            ]).map_err(log_err)?;
            stmt.next().map_err(log_err)?;

        };


        let query = "INSERT INTO Documents (path, term_count) VALUES (:path, :count)";
        let mut insert = self.connection.prepare(query).map_err(|err| {
            eprintln!("ERROR: Could not execute query {query}: {err}");
        })?;

        insert.bind((":path", path.to_str().unwrap())).map_err(log_and_ignore)?;
        insert.bind((":count", Lexer::new(content).count() as i64)).map_err(log_and_ignore)?;
        insert.next().map_err(log_and_ignore)?;
        Ok(())
    }
}

pub type DocFreq = HashMap<String, usize>;
pub type TermFreq = HashMap<String, usize>;
pub type TermFreqPerDoc = HashMap<PathBuf, (usize, TermFreq)>;

#[derive(Default, Deserialize, Serialize)]
pub struct InMemoryModel {
    pub tfpd: TermFreqPerDoc,
    pub df: DocFreq,
}

impl Model for InMemoryModel {
    fn search_query(&self, query: &[char]) -> Result<Vec<(PathBuf, f32)>, ()> {
        let tokens = Lexer::new(&query).collect::<HashSet<String>>();
        let mut results: Vec::<(PathBuf, f32)> = self.tfpd.iter().map(|(path, (n, tf_table))| {
            let mut rank = 0f32;
            for token in &tokens {
                rank += compute_tf(&token, *n, tf_table) * compute_idf(&token, self.tfpd.len(), &self.df);
            }
            (path.clone(), rank)
        }).filter(|(_, rank)| *rank > 0f32).collect();
        results.sort_by(|(_, rank1), (_, rank2)| rank2.partial_cmp(rank1).unwrap());
        Ok(results)
    }

    fn add_document(&mut self, file_path: PathBuf, content: &[char]) -> Result<(), ()> {
        let mut tf = TermFreq::new();
        let mut n = 0;
        for term in Lexer::new(&content) {
            if let Some(freq) = tf.get_mut(&term) {
                *freq += 1;
            } else {
                tf.insert(term, 1);
            }
            n += 1;
        }

        for t in tf.keys() {
            if let Some(freq) = self.df.get_mut(t) {
                *freq += 1;
            } else {
                self.df.insert(t.into(), 1);
            }
        }

        self.tfpd.insert(file_path, (n, tf));
        Ok(())
    }
}

/// Term frequency 
///  tf(t,d), is the relative frequency of term t within document d
pub fn compute_tf(t: &str, n: usize, d: &TermFreq) -> f32 {
    // m:  f(t,d) is the raw count of a term in a document
    let m = d.get(t).cloned().unwrap_or(0) as f32;
    // n: sum of  the raw count of a term in a document
    let n = n as f32;
    m / n
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