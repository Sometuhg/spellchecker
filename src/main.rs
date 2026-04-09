use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;


fn load_dictionary(path: &str) -> io::Result<HashSet<String>> {
    let content = fs::read_to_string(path)?;
    let mut dictionary = HashSet::new();

    for line in content.lines() {
        
        let word = line.trim().to_lowercase();

        if !word.is_empty() {
            dictionary.insert(word);
        }
    }

    Ok(dictionary)
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    // Early exit for identical strings (including both empty)
    if s1 == s2 {
        return 0;
    }

    let len1 = s1.len();
    let len2 = s2.len();

    // Handle empty strings explicitly so we never risk underflow later
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

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1.as_bytes()[i - 1] == s2.as_bytes()[j - 1] {
                0
            } else {
                1
            };

            dp[i][j] = (dp[i - 1][j] + 1)          // delete
                .min(dp[i][j - 1] + 1)             // insert
                .min(dp[i - 1][j - 1] + cost);     // substitute
        }
    }

    dp[len1][len2]
}

fn find_misspelled_with_suggestions(text: &str, dictionary: &HashSet<String>) -> Vec<(String, Vec<String>)> {
    let mut results = Vec::new();

    for token in text.split_whitespace() {
        let normalized = token.trim().to_lowercase();

        if dictionary.contains(&normalized){
            continue;
        }

        let mut candidate: Vec<(usize, String)> = Vec::new();

        for word in dictionary.iter(){
            let distance = levenshtein_distance(&normalized, word);

            if distance > 0 {
                candidate.push((distance, word.clone()));
            }
        }

    candidate.sort_by_key(|(distance, _)| *distance);

    let suggestions = candidate.into_iter().take(3).map(|(_, word)| word).collect();
    results.push((normalized, suggestions))
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
    let dictionary = load_dictionary(dict_path)?;
    println!("Loaded {} words!", dictionary.len());
    
    println!("Reading text from {}...", input_path);
    let text = fs::read_to_string(input_path)?;   
    
        let results = find_misspelled_with_suggestions(&text, &dictionary);
    
    if results.is_empty() {
        println!("No misspelled words found!");
    } else {
        println!("Misspelled words found:");
        for (word, suggestions) in results {
            println!(" - \"{}\"", word);
            if suggestions.is_empty() {
                println!("   (no suggestions found)");
            } else {
                print!("   Suggestions: ");
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
