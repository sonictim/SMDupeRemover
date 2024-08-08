use rusqlite::{Connection, Result};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::error::Error;
use terminal_size::{Width, terminal_size};

const DEFAULT_ORDER: [&str; 12] = [
    "CASE WHEN pathname LIKE '%Audio Files%' THEN 1 ELSE 0 END ASC",
    "CASE WHEN pathname LIKE '%RECORD%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%CREATED SFX%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%CREATED FX%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%LIBRARY%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%PULLS%' THEN 0 ELSE 1 END ASC",
    "duration DESC",
    "channels DESC",
    "sampleRate DESC",
    "bitDepth DESC",
    "BWDate ASC",
    "scannedDate ASC",
];

const DEFAULT_TAGS: [&str; 29] = [
    "-Reverse_", "-RVRS_", "-A2sA_", "-Delays_", "-ZXN5_", "-NYCT_", "-PiSh_", "-PnT2_", "-7eqa_",
    "-Alt7S_", "-AVrP_", "-X2mA_", "-PnTPro_", "-M2DN_", "-PSh_", "-ASMA_", "-TmShft_", "-Dn_",
    "-DVerb_", "-spce_", "-RX7Cnct_", "-AVSt", "-VariFi", "-DEC4_", "-VSPD_", "-6030_", "-NORM_",
    "-AVrT_", "-RING_"
];




#[derive(Debug)]
struct Config {
    primary_db: Option<String>,
    compare_db: Option<String>,
    duplicate_db: bool,
    skip_filename_check: bool,
    group_sort: Option<String>,
    group_null: bool,
    prune_tags: bool,
    safe: bool,
    prompt: bool,
    verbose: bool,
}

impl Config {
    fn new(args: &[String]) -> Result<Config, &'static str> {
        let mut primary_db = None;
        let mut compare_db = None;
        let mut duplicate_db = false;
        let mut skip_filename_check = false;
        let mut group_sort = None;
        let mut group_null = false;
        let mut prune_tags = false;
        let mut safe_mode = true;
        let mut prompt = true;
        let mut verbose = false;

        let mut i = 1;
        while i < args.len() {
            match args[i] {
                "--generate-config-files" => generate_config_files(),
                "--prune-tags" => prune_tags = true,
                "--no-filename-check" => skip_filename_check = true,
                "--group-by-show" | "-s" => group = Some("show"),
                "--group-by-library" | "-l" => group = Some("library"),
                "--group" => {
                    if i + 1 < args.len() {
                        group = args[i + 1].as_str();
                        i += 1; // Skip the next argument since it's the database name
                    } else {
                        print_help();
                        return Ok(());
                    }
                },
                "--group-null" => {
                    if i + 1 < args.len() {
                        group = args[i + 1].as_str();
                        i += 1; // Skip the next argument since it's the database name
                        group_null = true;
                    } else {
                        print_help();
                        return Ok(());
                    }
                },
                "--compare" => {
                    if i + 1 < args.len() {
                        compare_db = check_path(args[i + 1]);
                        i += 1; // Skip the next argument since it's the database name
                    } else {
                        print_help();
                        return Err("Missing database name for --compare");
                    }
                },
                "--no-prompt" | "--yes" => prompt = false,
                "--unsafe" => {
                    safe_mode = false;
                    prompt = false;
                },
                "--create-duplicates-database" => create_dup_db = true,
                "--verbose" => verbose = true,
                "--help" => {
                    print_help();
                    return Err("Help requested");
                }
                _ => {
                    if args[i].starts_with('-') && !args[i].starts_with("--") {
                        for c in args[i][1..].chars() {
                            match c {
                                'g' => {
                                    if i + 1 < args.len() {
                                        group = args[i + 1].as_str();
                                        i += 1; // Skip the next argument since it's the database name
                                    } else {
                                        print_help();
                                        return Ok(());
                                    }
                                },
                                't' => prune_tags_flag = true,
                                'n' => no_filename_check = true,
                                's' => group = Some("show"),
                                'l' => group = Some("library"),
                                'y' => just_say_yes = true,
                                'u' => {
                                    unsafe_mode = true;
                                    just_say_yes = true;
                                },
                                'd' => duplicates_database = true,
                                'v' => verbose = true,
                                'h' => {
                                    print_help();
                                    return Ok(());
                                },
                                'c' => {
                                    if i + 1 < args.len() {
                                        compare_db = Some(args[i + 1].clone());
                                        i += 1; // Skip the next argument since it's the database name
                                    } else {
                                        print_help();
                                        return Ok(());
                                    }
                                },
                                _ => {
                                    println!("Unknown option: -{}", c);
                                    print_help();
                                    return Err("Unknown option");
                                }
                            }
                        }
                    } else {
                        if primary_db.is_none() {
                            primary_db = check_path(args[i]);

                        } else {
                            print_help();
                            return Err("Multiple primary databases specified");
                        }
                    }
                }
            }
            i += 1;
        }

        if primary_db.is_none() {
            print_help();
            return Err("No Primary Database Specified");
        }

        Ok(Config {
            primary_db,
            compare_db,
            duplicate_db,
            skip_filename_check,
            group_sort,
            group_null,
            prune_tags,
            safe_mode,
            prompt,
            verbose,
        })
    }
}

fn check_path(path: &str) -> Option<String> {
    if Path::new(path).exists() {
        Some(path)
    } else {
        None
    }

}


fn check_path_validity(path: &str) -> Result<(), Box<dyn Error>> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err("Invalid file path".into())
    }
}


fn fetch_filenames(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT filename FROM justinmetadata")?;
    let filenames: HashSet<String> = stmt.query_map([], |row| row.get(0))?
        .filter_map(Result::ok)
        .collect();
    Ok(filenames)
}

fn fetch_filenames_and_row_id(conn: &Connection) -> Result<HashMap<String, Vec<usize>>> {
    let mut stmt = conn.prepare("SELECT rowid, filename FROM justinmetadata")?;
    let mut filenames: HashMap<String, Vec<usize>> = HashMap::new();

    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let id = row.get(0)?;
        let filename = row.get(1)?;
        filenames.entry(filename)
                .or_insert_with(Vec::new)
                .push(id);
    }

    Ok(filenames)
}

fn delete_rows(conn: &mut Connection, rows: &HashSet<usize>, verbose: bool) -> Result<()> {
    let tx = conn.transaction()?;

    for row in rows.iter() {
        // Prepare and execute the query to get the filename
        let mut stmt = tx.prepare("SELECT filename FROM justinmetadata WHERE rowid = ?1")?;
        let mut rows = stmt.query(params![row])?;

        // Retrieve the filename if available
        let filename = if let Some(r) = rows.next()? {
            Some(r.get::<_, String>(0)?)
        } else {
            None
        };

        // Print details if verbose and filename is available
        if verbose {
            if let Some(name) = filename {
                println!("Deleting ID: {} Filename: {}", row, name);
            } else {
                println!("Deleting ID: {} Filename not found", row);
            }
        }

        // Execute the deletion
        let delete_query = "DELETE FROM justinmetadata WHERE rowid = ?1";
        tx.execute(delete_query, params![row])?;
    }
    
    
    tx.commit()?;
    conn.execute("VACUUM", [])?;

    Ok(())
}

// fn get_filename_by_id(conn: &Connection, id: usize) -> Result<Option<String>> {
//     let mut stmt = conn.prepare("SELECT filename FROM justinmetadata WHERE id = ?1")?;
//     let mut rows = stmt.query(params![id])?;
    
//     if let Some(row) = rows.next()? {
//         let filename: String = row.get(0)?;
//         Ok(Some(filename))
//     } else {
//         Ok(None)
//     }
// }



// fn parse_txt_file(file_path: &str) -> Result<Vec<String>, rusqlite::Error> {
//     let path = Path::new(file_path);
    
//     if path.exists() {
//         let file = File::open(&path).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
//         let reader = io::BufReader::new(file);
//         let tags: Vec<String> = reader.lines()
//             .filter_map(|line| {
//                 let line = line.ok()?;
//                 let trimmed_line = line.trim().to_string();
//                 if trimmed_line.is_empty() {
//                     None
//                 } else {
//                     Some(trimmed_line)
//                 }
//             })
//             .collect();
//         Ok(tags)
//     } 
//     else 
//     {
//         // Use DEFAULT_TAGS if the file doesn't exist
//         let default_tags: Vec<String> = DEFAULT_TAGS.iter().map(|&s| s.to_string()).collect();
//         Ok(default_tags)
//     }
// }



fn filter_hashmap(map: HashMap<String, Vec<usize>>) -> HashSet<usize> {
    let mut rows_to_delete: HashSet<usize> = HashSet::new();
    for (_key, values) in &map {
        if values.len() == 1 {continue;}
        for window in values.windows(2) {
            if let [a, b] = window {
                rows_to_delete.insert(filecompare(a,b));
            }
        }
    }
    rows_to_delete

}

fn filecompare(a: &usize, b: &usize) -> usize {
    if a > b {return *b}
    *a
}



fn main() -> Result<(), Box<dyn Error>> {

    let args: Vec<String> = env::args().collect();
    let config = Config::new(&args)?;


    println!("CREATE DBs and OPEN THEM");
    let mut primary_conn = Connection::open(config.primary_db.unwrap())?;
    let primary_files = fetch_filenames_and_row_id(&primary_conn).unwrap();
    println!("CHECK FOR DUPLICATES");
    let rows_to_delete = filter_hashmap(primary_files);
    println!("Found {} Duplicates", rows_to_delete.len());
    println!("REMOVE DUPLICATES");
    delete_rows(&mut primary_conn, &rows_to_delete, false)?;
    
    Ok(())
}


fn print_help() {
    let help_message = "
Usage: SMDupeRemover <database> [options]

Options:
    -c, --compare <database>          Compare with another database
    -d, --create-duplicates-database  Generates an additional _dupes database of all files that were removed
    -g, --generate-config-files       Generate default config files (SMDupe_order.txt and SMDupe_tags.txt)
    -h, --help                        Display this help message
    -n, --no-filename-check           Skips searching for filename duplicates in main database
    -t, --prune-tags                  Remove Files with Specified Tags in SMDupe_tags.txt or use defaults
    -u, --unsafe                      WRITES DIRECTLY TO TARGET DATABASE with NO PROMPT
    -v, --verbose                     Display Additional File Processing Details
    -y, --no-prompt                   Auto Answer YES to all prompts

Arguments:
    <database>                        Path to the primary database

Examples:
    smduperemover mydatabase.sqlite --prune-tags
    smduperemover mydatabase.sqlite -p -g
    smduperemover mydatabase.sqlite -pvu
    smduperemover mydatabase.sqlite --compare anotherdatabase.sqlite

Configuration:
    SMDupe_order.txt defines the order of data (colums) checked when deciding on the logic of which file to keep
    SMDupe_tags.txt is a list of character combinations that if found in the filename, it will be removed with the -p option

Description:
    SMDupeRemover is a tool for removing duplicate filename entries from a Soundminer database.
    It can generate configuration files, prune tags, and compare databases.
";

    println!("{}", help_message);
}

fn generate_config_files() {
    // Generate SMDupe_order.txt and SMDupe_tags.txt with default values
    let order_file_path = "SMDupe_order.txt";

    let mut order_file = File::create(order_file_path).unwrap();
    writeln!(order_file, "## Column in order of Priority and whether it should be DESCending or ASCending.  Hashtag will bypass")?;
    for field in &DEFAULT_ORDER {
        writeln!(order_file, "{}", field).unwrap();
    }

    println!("Created {} with default order.", order_file_path);

    let tags_file_path = "SMDupe_tags.txt";

    let mut tags_file = File::create(tags_file_path).unwrap();
    for tag in DEFAULT_TAGS {
        writeln!(tags_file, "{}", tag).unwrap();
    }

    println!("Created {} with default tags.", tags_file_path);
        // return Ok(());
}

