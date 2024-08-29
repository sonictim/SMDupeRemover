#![allow(non_snake_case)]
use rusqlite::{Connection, Result};
use std::collections::HashSet;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::error::Error;
// use terminal_size::{Width, terminal_size};
use regex::Regex;
// use ordered_float::OrderedFloat;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const BATCH_SIZE: usize = 12321;

const DEFAULT_ORDER: [&str; 6] = [
    "duration DESC",
    "channels DESC",
    "sampleRate DESC",
    "bitDepth DESC",
    "BWDate ASC",
    "scannedDate ASC",
];

const DEFAULT_TAGS: [&str; 46] = [
    "-1eqa_",
    "-6030_", 
    "-7eqa_",
    "-A2sA_", 
    "-A44m_", 
    "-A44s_", 
    "-Alt7S_", 
    "-ASMA_", 
    "-AVrP_", 
    "-AVrT_", 
    "-AVSt", 
    "-DEC4_", 
    "-Delays_", 
    "-Dn_",
    "-DUPL_",
    "-DVerb_", 
    "-GAIN_", 
    "-M2DN_", 
    "-NORM_",
    "-NYCT_", 
    "-PiSh",
    "  PI SH ",  
    "-PnT2_", 
    "-PnTPro_", 
    "-ProQ2_", 
    "-PSh_", 
    "-Reverse_", 
    "-RVRS_", 
    "-RING_", 
    "-RX7Cnct_", 
    "-spce_", 
    "-TCEX", 
    "-TiSh", 
    "-TmShft_", 
    "-VariFi", 
    "-VlhllVV_", 
    "-VSPD_",
    "-VitmnMn_", 
    "-VtmnStr_", 
    "-X2mA_", 
    "-X2sA_", 
    "-XForm_",
    "-Z2N5_",
    "-Z2S5_",
    "-Z4n2_",
    "-ZXN5_", 
];

const ORDER_FILE_PATH: &str = "SMDupe_Order.txt";
const TAG_FILE_PATH: &str = "SMDupe_tags.txt";



#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct FileRecord {
    id: usize,
    filename: String,
    duration: String,
}

#[derive(Debug)]
struct Config {
    target_db: Option<String>,
    compare_db: Option<String>,
    duplicate_db: bool,
    filename_check: bool,
    group_sort: Option<String>,
    group_null: bool,
    numbers_check: bool,
    prune_tags: bool,
    safe: bool,
    prompt: bool,
    verbose: bool,
}

impl Config {
    fn new(args: &[String]) -> Result<Config, &'static str> {
        let mut target_db = None;
        let mut compare_db: Option<String> = None;
        let mut duplicate_db = false;
        let mut filename_check = true;
        let mut group_sort: Option<String> = None;
        let mut group_null = false;
        let mut numbers_check = false;
        let mut prune_tags = false;
        let mut safe = true;
        let mut prompt = true;
        let mut verbose = false;
        let mut config_gen = false;
        
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--generate-config-files" => {generate_config_files(false).unwrap(); config_gen = true;},
                "--tjf" => {generate_config_files(true).unwrap(); config_gen = true;},
                "--all" => {
                    prune_tags = true;
                    numbers_check = true;
                    duplicate_db = true;
                    filename_check = true;
                }
                "--prune-tags" => prune_tags = true,
                "--deep-dive" => numbers_check = true,
                "--no-filename-check" => filename_check = false,
                "--group-by-show" | "-s" => group_sort = Some("show".to_string()),
                "--group-by-library" | "-l" => group_sort = Some("library".to_string()),
                "--group" => {
                    if i + 1 < args.len() {
                        group_sort = Some(args[i + 1].clone());
                        i += 1; // Skip the next argument since it's the database name
                    } else {
                        print_help();
                        return Err("group argument missing");
                    }
                },
                "--group-null" => {
                    if i + 1 < args.len() {
                        group_sort = Some(args[i + 1].clone());
                        i += 1; // Skip the next argument since it's the database name
                        group_null = true;
                    } else {
                        print_help();
                        return Err("group argument missing");
                    }
                },
                "--compare" => {
                    if i + 1 < args.len() {
                        compare_db = check_path(args[i + 1].as_str());
                        i += 1; // Skip the next argument since it's the database name
                    } else {
                        print_help();
                        return Err("Missing database name for --compare");
                    }
                },
                "--no-prompt" | "--yes" => prompt = false,
                "--unsafe" => {
                    safe = false;
                    prompt = false;
                },
                "--create-duplicates-database" => duplicate_db = true,
                "--verbose" => verbose = true,
                "--help" => {
                    print_help();
                    return Err("Help requested");
                }
                _ => {
                    if args[i].starts_with('-') && !args[i].starts_with("--") {
                        for c in args[i][1..].chars() {
                            match c {
                                'a' => {
                                    prune_tags = true;
                                    numbers_check = true;
                                    duplicate_db = true;
                                    filename_check = true;
                                }
                                'A' => {
                                    prune_tags = true;
                                    numbers_check = true;
                                    duplicate_db = true;
                                    filename_check = true;
                                    prompt = false;
                                }
                                'g' => {
                                    if i + 1 < args.len() {
                                        group_sort = Some(args[i + 1].clone());
                                        i += 1; // Skip the next argument since it's the database name
                                    } else {
                                        print_help();
                                        return Err("group argument missing");
                                    }
                                },
                                'i' => group_null = true,
                                't' => prune_tags = true,
                                'n' => filename_check = false,
                                's' => group_sort = Some("show".to_string()),
                                'l' => group_sort = Some("library".to_string()),
                                'y' => prompt = false,
                                'u' => {
                                    safe = false;
                                    prompt = false;
                                },
                                'd' => duplicate_db = true,
                                'v' => verbose = true,
                                'h' => {
                                    print_help();
                                    return Err("Asked for Help");
                                },
                                'c' => {
                                    if i + 1 < args.len() {
                                        compare_db = check_path(args[i + 1].as_str());
                                        i += 1; // Skip the next argument since it's the database name
                                    } else {
                                        print_help();
                                        return Err("Missing database name for --compare");
                                    }
                                },
                                '#'|'D' => numbers_check = true,
                                _ => {
                                    println!("Unknown option: -{}", c);
                                    print_help();
                                    return Err("Unknown option");
                                }
                            }
                        }
                    } else {
                        if target_db.is_none() {
                            target_db = check_path(args[i].as_str());

                        } else {
                            print_help();
                            return Err("Multiple primary databases specified");
                        }
                    }
                }
            }
            i += 1;
        }

        if target_db.is_none() {
            if !config_gen {print_help();}
            return Err("No Primary Database Specified");
        }

        Ok(Config {
            target_db,
            compare_db,
            duplicate_db,
            filename_check,
            group_sort,
            group_null,
            numbers_check,
            prune_tags,
            safe,
            prompt,
            verbose,
        })
    }
}

fn check_path(path: &str) -> Option<String> {
    if Path::new(path).exists() {
        Some(path.to_string())
    } else {
        None
    }

}
 

// GET FUNCTIONS
fn get_order(file_path: &str) -> Result<Vec<String>, io::Error> {
    let path = Path::new(file_path);

    if path.exists() {
        let file = File::open(path)?;
        let reader = io::BufReader::new(file);

        let lines: Vec<String> = reader.lines()
            .filter_map(|line| line.ok())
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        Ok(lines)
    } else {
        // If the file doesn't exist, return DEFAULT_ORDER
        Ok(DEFAULT_ORDER.iter().map(|&s| s.to_string()).collect())
    }
}

fn get_tags(file_path: &str) -> Result<Vec<String>, rusqlite::Error> {
    println!("Gathering tags to search for");
    let path = Path::new(file_path);
    
    if path.exists() {
        let file = File::open(&path).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        let reader = io::BufReader::new(file);
        let tags: Vec<String> = reader.lines()
            .filter_map(|line| {
                let line = line.ok()?;
                let trimmed_line = line.trim().to_string();
                if trimmed_line.is_empty() {
                    None
                } else {
                    Some(trimmed_line)
                }
            })
            .collect();
        Ok(tags)
    } else {
        // Use DEFAULT_TAGS if the file doesn't exist
        let default_tags: Vec<String> = DEFAULT_TAGS.iter().map(|&s| s.to_string()).collect();
        Ok(default_tags)
    }
}

fn get_connection_source_filepath(conn: &Connection) -> String {
    let path = conn.path().unwrap(); // This gives you a &Path
    let path_str = path.to_str().unwrap().to_string().replace("_thinned", ""); // Converts &Path to String
    path_str
}

fn get_db_size(conn: &Connection,) -> usize {

    // Get the count of remaining rows
     let count: usize = conn.query_row(
         "SELECT COUNT(*) FROM justinmetadata",
         [],
         |row| row.get(0) 
     ).unwrap();
     count
}

fn get_root_filename(filename: &str) -> Option<String> {
    // Use regex to strip off trailing pattern like .1, .M, but preserve file extension
    let re = Regex::new(r"^(?P<base>.+?)(\.\d+|\.\w+)+(?P<ext>\.\w+)$").unwrap();
    if let Some(caps) = re.captures(filename) {
        Some(format!("{}{}", &caps["base"], &caps["ext"]))
    } else {
        // If no match, return the original filename
        Some(filename.to_string())
    }
}



// DUPLICATES DB
fn create_duplicates_db(source_db_path: &str, dupe_records_to_keep: &HashSet<FileRecord>) -> Result<()> {
    println!("Generating Duplicates Only Database.  This can take awhile.");
    let duplicate_db_path = format!("{}_dupes.sqlite", &source_db_path.trim_end_matches(".sqlite"));
    fs::copy(&source_db_path, &duplicate_db_path).unwrap();
    let mut dupe_conn = Connection::open(&duplicate_db_path)?;
    
    let mut dupe_records_to_delete = fetch_filerecords_from_database(&dupe_conn)?;
    dupe_records_to_delete.retain(|record| !dupe_records_to_keep.contains(record));
    
    delete_file_records(&mut dupe_conn, &dupe_records_to_delete, false)?;
    vacuum_db(&dupe_conn)?;

    println!("{} records moved to {}", get_db_size(&dupe_conn), duplicate_db_path);

    Ok(())
}


//FETCH FUNCTIONS
fn fetch_filerecords_from_database(conn: &Connection) -> Result<HashSet<FileRecord>> {
    println!("Gathering records from {}", get_connection_source_filepath(&conn));
    let mut stmt = conn.prepare("SELECT rowid, filename, duration FROM justinmetadata")?;
    let file_records: HashSet<FileRecord> = stmt.query_map([], |row| {
        Ok(FileRecord {
            id: row.get(0)?,
            filename: row.get(1)?,
            duration: row.get(2)?,
        })
    })?
    .filter_map(Result::ok)
    .collect();

    Ok(file_records)
}

fn extract_filenames_set_from_records(file_records: &HashSet<FileRecord>) -> HashSet<String> {
    file_records.iter().map(|record| record.filename.clone()).collect()
}


// GATHER FUNCTIONS
fn gather_compare_database_overlaps(target_conn: &Connection, compare_conn: &Connection) -> Result<HashSet<FileRecord>> {
    
    let compare_records = fetch_filerecords_from_database(&compare_conn)?;
    let filenames_to_check = extract_filenames_set_from_records(&compare_records);
    let mut matching_records = fetch_filerecords_from_database(&target_conn)?;
    println!("Comparing filenames between {} and {}", target_conn.path().unwrap().display(), compare_conn.path().unwrap().display());
    matching_records.retain(|record| filenames_to_check.contains(&record.filename));

    if matching_records.is_empty() {
        println!("NO OVERLAPPING FILE RECORDS FOUND!");
    } else {
        println!(
            "Found {} overlapping file records between {} and {}.",
            matching_records.len(),
            get_connection_source_filepath(&target_conn),
            get_connection_source_filepath(&compare_conn)
        );
    }

    Ok(matching_records)
}

fn gather_duplicate_filenames_in_database(conn: &mut Connection, group_sort: &Option<String>, group_null: bool, verbose: bool) -> Result<HashSet<FileRecord>, rusqlite::Error> {
    println!("Searching {} for duplicate records", get_connection_source_filepath(&conn));
    let mut file_records = HashSet::new();
    let order = get_order(ORDER_FILE_PATH).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
    if verbose {
        for line in &order {println!("{}", line);}
    }

    // Construct the ORDER BY clause dynamically
    let order_clause = order.join(", ");

    // Build the SQL query based on whether a group_sort is provided
    let (partition_by, where_clause) = match group_sort {
        Some(group) => {
            println!("Grouping duplicate record search by {}", group);
            
            let where_clause = if group_null {
                println!("Records without a {} entry will be processed together.", group);
                String::new()
            } else {
                println!("Records without a {} entry will be skipped.", group);
                format!("WHERE {} IS NOT NULL AND {} != ''", group, group)
            };
            
            (format!("{}, filename", group), where_clause)
        }
        None => ("filename".to_string(), String::new()),
    };
    
    let sql = format!(
        "
        WITH ranked AS (
            SELECT
                rowid AS id,
                filename,
                duration,
                ROW_NUMBER() OVER (
                    PARTITION BY {}
                    ORDER BY {}
                ) as rn
            FROM justinmetadata
            {}
        )
        SELECT id, filename, duration FROM ranked WHERE rn > 1
        ",
        partition_by, order_clause, where_clause
    );
    
    let total_overlaps = count_total_duplicate_filenames(conn).unwrap();
    let unique_filenames = count_unique_duplicate_filenames(conn).unwrap();
    let total = total_overlaps - unique_filenames;
    
    if verbose {
        println!("SQL found {} duplicate records with {} unique filenames", total_overlaps, unique_filenames);
    }
    println!("{} records can be removed", total);
    println!("Processing which filenames are best for removal");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(FileRecord {
            id: row.get(0)?,
            filename: row.get(1)?,
            duration: row.get(2)?,
        })
    })?;

    let mut counter: usize = 1;

    for file_record in rows {
        file_records.insert(file_record?);
        let _ = io::stdout().flush();
        print!("\r{} / {}", counter, total);
        counter += 1;
    }
    println!("\nMarked {} duplicate records for deletion.", file_records.len());

    Ok(file_records)
}

fn gather_records_with_trailing_numbers(conn: &mut Connection, total: usize) -> Result<HashSet<FileRecord>, rusqlite::Error> {
    println!("Performing Deep Dive Search for Similar Records ending with .1 (or multiple numbers) or .M");

    let mut file_records = HashSet::new();
    let mut file_groups: HashMap<String, Vec<FileRecord>> = HashMap::new();

    let mut stmt = conn.prepare("SELECT rowid, filename, duration FROM justinmetadata")?;
    let rows = stmt.query_map([], |row| {
        Ok(FileRecord {
            id: row.get(0)?,
            filename: row.get(1)?,
            duration: row.get(2)?,
        })
    })?;

    println!("Analyzing Records");
    let mut counter: usize = 1;

    for row in rows {
        let file_record = row?;
        
        let base_filename = get_root_filename(&file_record.filename)
            .unwrap_or_else(|| file_record.filename.clone());

        let _ = io::stdout().flush();
        print!("\r{} / {}", counter, total);
        counter += 1;

        file_groups
            .entry(base_filename)
            .or_insert_with(Vec::new)
            .push(file_record);
    }

    for (root, records) in file_groups {
 
        if records.len() <= 1 {
            continue;
        }
        
        let root_found = records.iter().any(|record| record.filename == root);
        
        if root_found {
            file_records.extend(
                records.into_iter().filter(|record| record.filename != root)
            );
        } else {
            file_records.extend(
                records.into_iter().skip(1)
            );
        }
    }

    println!("\nFound {} total records ending in .1 or .M", file_records.len());

    Ok(file_records)
}


fn gather_filenames_with_tags(conn: &mut Connection, verbose: bool) -> Result<HashSet<FileRecord>> {
    println!("Searching {} for filenames containing tags", get_connection_source_filepath(&conn));
    let mut file_records = HashSet::new();

    let tags = get_tags(TAG_FILE_PATH)?;

    let mut tags_found: usize = 0;
    for tag in tags {
        let query = format!("SELECT rowid, filename, duration FROM justinmetadata WHERE filename LIKE '%' || ? || '%'");
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map([tag.clone()], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                filename: row.get(1)?,
                duration: row.get(2)?,
            })
        })?;

        for file_record in rows {
            let file_record = file_record?;
            file_records.insert(file_record);
        }

        if verbose && !file_records.is_empty() {
            println!("Filenames found for tag '{}': {}", tag, file_records.len()-tags_found);
            tags_found = file_records.len();
        }
    }
    println!("{} total records containing tags marked for deletion", file_records.len());
    Ok(file_records)
}


// DELETE FUNCTION
fn delete_file_records(conn: &mut Connection, records: &HashSet<FileRecord>, verbose: bool) -> Result<()> {
    let mut counter = 1;
    let total = records.len();
    println!("Removing Records Marked as Duplicates");
    let tx = conn.transaction()?;

    let mut sorted_records: Vec<_> = records.iter().collect();
    sorted_records.sort_by(|a, b| b.id.cmp(&a.id));  // Sort by ID in descending order

    sorted_records
        .chunks(BATCH_SIZE)
        .try_for_each(|chunk| {
            if verbose {
                for record in chunk {
                    println!("\rDeleting ID: {}, Filename: {}", record.id, record.filename);
                }
            } else {
                let _ = io::stdout().flush();
                print!("\r{} / {}", counter, total);
                counter += BATCH_SIZE;
            }
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            let query = format!("DELETE FROM justinmetadata WHERE rowid IN ({})", placeholders);
            let params: Vec<&dyn rusqlite::types::ToSql> = chunk.iter().map(|record| &(record.id) as &dyn rusqlite::types::ToSql).collect();
            tx.execute(&query, params.as_slice()).map(|_| ())
    })?;

    println!("\r{} / {}", total, total);
    tx.commit()?;

    Ok(())
}


fn main() -> Result<(), Box<dyn Error>> {
    println!("SMDupeRemover v{}", VERSION);

    let args: Vec<String> = env::args().collect();
    let config = Config::new(&args)?;

    let source_db_path = &config.target_db.unwrap();
    println!("Opening {}", source_db_path);
    let mut conn = Connection::open(&source_db_path)?; 

    let total_records = get_db_size(&conn);
    println!("{} Total Records found in {}", total_records, source_db_path);
  
    let mut all_ids_to_delete = HashSet::<FileRecord>::new();

    if let Some(compare_db_path) = config.compare_db {
        let compare_conn = Connection::open(&compare_db_path)?; 
        let ids_from_compare_db = gather_compare_database_overlaps(&conn, &compare_conn)?;
        all_ids_to_delete.extend(ids_from_compare_db);
    }

    if config.filename_check {
        let ids_from_duplicates = gather_duplicate_filenames_in_database(&mut conn, &config.group_sort, config.group_null, config.verbose)?;
        all_ids_to_delete.extend(ids_from_duplicates);
    }

    if config.prune_tags {
        let ids_from_tags = gather_filenames_with_tags(&mut conn, config.verbose)?;
        all_ids_to_delete.extend(ids_from_tags);
    }
    
    if config.numbers_check {
        let number_dupes = gather_records_with_trailing_numbers(&mut conn, total_records)?;
        all_ids_to_delete.extend(number_dupes);
    }

    if all_ids_to_delete.is_empty() {
        println!("No files to delete.");
        return Ok(());
    }

    print!("Found {} total records to delete. ", all_ids_to_delete.len());
    if config.prompt {
        println!(" Type 'yes' to confirm deletion: ");
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        if user_input.trim().to_lowercase() != "yes" {
            println!("Deletion aborted.");
            return Ok(());
        }

    } 
    
    if config.duplicate_db && all_ids_to_delete.len() > 0 {
        create_duplicates_db(&source_db_path, &all_ids_to_delete)?;
    }
    
    let work_db_path = format!("{}_thinned.sqlite", &source_db_path.trim_end_matches(".sqlite"));
    if config.safe {
        println!("Backing up {}", source_db_path);
        fs::copy(&source_db_path, &work_db_path)?;
        conn = Connection::open(&work_db_path).unwrap();
    }
    println!("Proceeding with deletion."); 
    
    delete_file_records(&mut conn, &all_ids_to_delete, config.verbose)?;
    vacuum_db(&conn)?;
    println!("Removed {} records.", all_ids_to_delete.len());


    if config.safe {
        println!("Thinned records database moved to: {}", work_db_path);
    } else {    
        println!("Database {} sucessfully thinned", source_db_path);
    }

    Ok(())
}

fn vacuum_db(conn: &Connection) -> Result<()> { 
    println!("Cleaning up Database {}", get_connection_source_filepath(&conn));
    conn.execute("VACUUM", [])?; // Execute VACUUM on the database
    Ok(())
}


fn print_help() {
    let help_message = "
Usage: SMDupeRemover <database> [options]

Options:
    -a, --all                         Do all the things: Standard dupliate search, tag search, deep dive, create duplicate database
    -A,                               Same as '-ay' do all the things and no prompt for deletions
    -c, --compare <database>          Compare with another database
    -d, --create-duplicates-database  Generates an additional _dupes database of all files that were removed
    -D, --deep-dive                   Perform a 'deep dive'duplicates search.  Looking for similar files with .1 or .M before the extension
        --generate-config-files       Generate default config files (SMDupe_order.txt and SMDupe_tags.txt)
    -g, --group <column>              Search for Duplicates within the specified column groupings.  NULL column records skipped
        --group-null <column>         Search for Duplicates within the specified column groupings.  NULL column records processed together
    -h, --help                        Display this help message
    -i,                               used in conjunction with -s or -l to include null grouping
    -l, --group-by-library            Search for duplicates within each Library. Untagged Library files skipped
    -n, --no-filename-check           Skips searching for filename duplicates in main database
    -s, --group-by-show               Search for duplicates within each show. Untagged Show files skipped
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

fn generate_config_files(tjf: bool) -> Result<()> {
    if tjf {
        create_tjf_order_file()?;
    } else {
        create_order_file()?;
    }
    
    let mut tags_file = File::create(TAG_FILE_PATH).unwrap();
    writeln!(tags_file, "## Any Text String, if found in the filename, will mark it for deletion.").unwrap();
    writeln!(tags_file, "## Protools Audio Suite tags with -????_ so these defaults will target those tags.").unwrap();
    for tag in DEFAULT_TAGS {
        writeln!(tags_file, "{}", tag).unwrap();
    }
    if tjf {
        writeln!(tags_file, ".wav.").unwrap();
        writeln!(tags_file, ".aiff.").unwrap();
        writeln!(tags_file, ".new.").unwrap();
    }
    println!("Created {} with default tags.", TAG_FILE_PATH);
    Ok(())
}


fn create_order_file() -> Result<()> {
    let mut order_file = File::create(ORDER_FILE_PATH).unwrap();
    writeln!(order_file, "## Column in order of Priority and whether it should be DESCending or ASCending.  Hashtag will bypass").unwrap();
    writeln!(order_file, "## These are SQL arguments and Google/ChatGPT can help you figure out how to compose them").unwrap();
    writeln!(order_file, "## ").unwrap();
    writeln!(order_file, "## Custom Examples:").unwrap();
    writeln!(order_file, "## CASE WHEN pathname LIKE '%Audio Files%' THEN 1 ELSE 0 END ASC").unwrap();
    writeln!(order_file, "## ^----- Records with Audio Files in the path will be removed over something that does not have it.").unwrap();
    writeln!(order_file, "## CASE WHEN pathname LIKE '%RECORD%' THEN 0 ELSE 1 END ASC").unwrap();
    writeln!(order_file, "## ^----- Records with RECORD (not case sensitive) in the path will be kept over records without").unwrap();
    writeln!(order_file, "## ").unwrap();
    writeln!(order_file, "").unwrap();
    for field in &DEFAULT_ORDER {
        writeln!(order_file, "{}", field).unwrap();
    }
    println!("Created {} with default order.", ORDER_FILE_PATH);
    Ok(())
    
}

fn create_tjf_order_file() -> Result<()> {
    let mut order_file = File::create(ORDER_FILE_PATH).unwrap();
    for field in &TJF_DEFAULT_ORDER {
        writeln!(order_file, "{}", field).unwrap();
    }
    println!("Created {} with TJF default order.", ORDER_FILE_PATH);
    Ok(())
}


const TJF_DEFAULT_ORDER: [&str; 22] = [
    "CASE WHEN pathname LIKE '%TJF RECORDINGS%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%LIBRARIES%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%SHOWS/Tim Farrell%' THEN 1 ELSE 0 END ASC",
    "CASE WHEN Description IS NOT NULL AND Description != '' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%Audio Files%' THEN 1 ELSE 0 END ASC",
    "CASE WHEN pathname LIKE '%RECORD%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%CREATED SFX%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%CREATED FX%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%LIBRARY%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%/LIBRARY%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%LIBRARY/%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%SIGNATURE%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%PULLS%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%EDIT%' THEN 1 ELSE 0 END ASC",
    "CASE WHEN pathname LIKE '%MIX%' THEN 1 ELSE 0 END ASC",
    "CASE WHEN pathname LIKE '%SESSION%' THEN 1 ELSE 0 END ASC",
    "duration DESC",
    "channels DESC",
    "sampleRate DESC",
    "bitDepth DESC",
    "BWDate ASC",
    "scannedDate ASC",
];



fn count_unique_duplicate_filenames(conn: &Connection) -> Result<usize> {
    let mut stmt = conn.prepare("
        SELECT COUNT(*)
        FROM (
            SELECT filename
            FROM justinmetadata
            GROUP BY filename
            HAVING COUNT(*) > 1
        )
    ")?;

    let count: usize = stmt.query_row([], |row| row.get(0))?;

    Ok(count)
}


fn count_total_duplicate_filenames(conn: &Connection) -> Result<usize> {
    let mut stmt = conn.prepare("
        SELECT SUM(occurrence_count)
        FROM (
            SELECT COUNT(*) AS occurrence_count
            FROM justinmetadata
            GROUP BY filename
            HAVING COUNT(*) > 1
        )
    ")?;

    let count: usize = stmt.query_row([], |row| row.get(0))?;

    Ok(count)
}
