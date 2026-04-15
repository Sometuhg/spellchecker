use std::env;
use std::fs;
use std::io;
use memmap2::Mmap; 
use std::collections::*;

type Dict = Vec<(usize, usize)>;


fn load_dictionary(path: &str) -> io::Result<(Mmap, Dict, HashMap<String, usize>)> {
    let file = fs::File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    let mut entries: Dict = Vec::with_capacity(400_000);
    let mut lookup_map: HashMap<String, usize> = HashMap::with_capacity(400_000);

    let mut start = 0;
    for i in 0..mmap.len() {
        if mmap[i] == b'\n' || mmap[i] == b'\r' {
            if start < i {
                let word = unsafe { std::str::from_utf8_unchecked(&mmap[start..i]) }.trim();

                if !word.is_empty() {
                    entries.push((start, i - start));
                    lookup_map.insert(word.to_string(), 1); // default frequency
                }
            }
            start = i + 1;
        }
    }

    // Last line
    if start < mmap.len() {
        let word = unsafe { std::str::from_utf8_unchecked(&mmap[start..]) }.trim();
        if !word.is_empty() {
            entries.push((start, mmap.len() - start));
            lookup_map.insert(word.to_string(), 1);
        }
    }

    println!("Dictionary loaded: {} words (memmap2 zero-copy + fast lookup map)", entries.len());
    Ok((mmap, entries, lookup_map))
}

fn get_freq(lookup_map: &HashMap<String, usize>, word: &str) -> Option<usize> {
    lookup_map.get(word).copied()
}

fn generate_candidates(word: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();

    // 1. Deletions - always cheap and very useful
    for i in 0..len {
        let mut cand = chars.clone();
        cand.remove(i);
        if !cand.is_empty() {
            candidates.push(cand.into_iter().collect());
        }
    }

    // 2. Transpositions - the most common real typo, keep all
    for i in 0..len.saturating_sub(1) {
        let mut cand = chars.clone();
        cand.swap(i, i + 1);
        candidates.push(cand.into_iter().collect());
    }

    // 3. Substitutions - only for short words (limit explosion)
    if len <= 7 {
        for i in 0..len {
            for c in 'a'..='z' {
                if c != chars[i] {
                    let mut cand = chars.clone();
                    cand[i] = c;
                    candidates.push(cand.into_iter().collect());
                }
            }
        }
    }

    // 4. Insertions - only for very short words (most expensive)
    if len <= 5 {
        for i in 0..=len {
            for c in 'a'..='z' {
                let mut cand = chars.clone();
                cand.insert(i, c);
                candidates.push(cand.into_iter().collect());
            }
        }
    }

    candidates.sort();
    candidates.dedup();
    candidates.retain(|c| !c.is_empty() && c != word);

    candidates
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    if s1 == s2 {
        return 0;
    }

    let len1 = s1.len();
    let len2 = s2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut dp = vec![vec![0usize; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        dp[i][0] = i;
    }
    for j in 0..=len2 {
        dp[0][j] = j;
    }

    let bytes1 = s1.as_bytes();
    let bytes2 = s2.as_bytes();

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if bytes1[i - 1] == bytes2[j - 1] { 0 } else { 1 };

            let mut min = (dp[i - 1][j] + 1)          // delete
                .min(dp[i][j - 1] + 1)                // insert
                .min(dp[i - 1][j - 1] + cost);        // substitute

            if i > 1 && j > 1 && bytes1[i - 1] == bytes2[j - 2] && bytes1[i - 2] == bytes2[j - 1] {
                min = min.min(dp[i - 2][j - 2] + 1);
            }

            dp[i][j] = min;
        }
    }

    dp[len1][len2]
}


fn tokenizor(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    for line in text.lines(){
        for word in line.split_whitespace() {

            let cleaned = word.trim_matches(|c: char|c.is_ascii_punctuation() && c != '\'' && c != '-').to_string();

            if cleaned.is_empty(){ continue;}

            let lookup = cleaned.replace("'", "").replace("-", "").to_lowercase();

            if !lookup.is_empty(){
                tokens.push(lookup);
            }
       }
    }

   tokens    
}

fn find_misspelled_with_suggestions(
    text: &str,
    lookup_map: &HashMap<String, usize>,   // fast O(1) lookup
) -> Vec<(String, Vec<String>)> {
    let mut results = Vec::new();
    let tokens = tokenizor(text);

    for normalized in tokens {
        if lookup_map.contains_key(&normalized) {
            continue;   // exact match, no suggestion needed
        }

        let candidates = generate_candidates(&normalized);

        let mut scored: Vec<(usize, usize, String)> = Vec::new();
        for cand in candidates {
            if let Some(freq) = lookup_map.get(&cand) {
                let dist = levenshtein_distance(&normalized, &cand);
                scored.push((dist, *freq, cand));
            }
        }

        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.cmp(&a.1))
                .then(a.2.cmp(&b.2))
        });

        let suggestions: Vec<String> = scored.into_iter().take(3).map(|(_, _, w)| w).collect();

        if !suggestions.is_empty() {
            results.push((normalized, suggestions));
        }
    }

    results
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
   
    if args.len() < 3 {
        eprintln!("Usage: {} <dictionary.txt> <input.txt>", args[0]);
        std::process::exit(1);
    }
    
    let dict_path = &args[1];
    let input_path = &args[2];
   
    println!("Loading dictionary from {}...", dict_path);
    
    // Load dictionary + pre-computed correction cache for short words
    let (_arena, dictionary, lookup_map) = load_dictionary(dict_path)?;
    println!("Loaded {} words!", dictionary.len());

    let text = fs::read_to_string(input_path)?;

    let results = find_misspelled_with_suggestions(&text, &lookup_map);
    if results.is_empty() {
        println!("No misspelled words found!");
    } else {
        println!("Misspelled words found:");
        for (word, suggestions) in results {
            println!(" - \"{}\"", word);
            if suggestions.is_empty() {
                println!(" (no suggestions found)");
            } else {
                print!(" Suggestions: ");
                for (i, sug) in suggestions.iter().enumerate() {
                    if i > 0 { print!(", "); }
                    print!("{}", sug);
                }
                println!();
            }
        }
    }
    Ok(())
}
