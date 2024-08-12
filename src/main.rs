use rusqlite::{Connection, Result};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::error::Error;
// use terminal_size::{Width, terminal_size};

const VERSION: &str = "0.1.5";

const BATCH_SIZE: usize = 10000;

const DEFAULT_ORDER: [&str; 6] = [
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

const ORDER_FILE_PATH: &str = "SMDupe_Order.txt";
const TAG_FILE_PATH: &str = "SMDupe_tags.txt";



#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct FileRecord {
    id: usize,
    filename: String,
}

#[derive(Debug)]
struct Config {
    target_db: Option<String>,
    compare_db: Option<String>,
    duplicate_db: bool,
    filename_check: bool,
    group_sort: Option<String>,
    group_null: bool,
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
                "--prune-tags" => prune_tags = true,
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
    println!("Determining logic for which record to keep");
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

    println!("\n{} records moved to {}", get_db_size(&dupe_conn), duplicate_db_path);

    Ok(())
}


//FETCH FUNCTIONS
fn fetch_filerecords_from_database(conn: &Connection) -> Result<HashSet<FileRecord>> {
    println!("Gathering records from {}", get_connection_source_filepath(&conn));
    let mut stmt = conn.prepare("SELECT rowid, filename FROM justinmetadata")?;
    let file_records: HashSet<FileRecord> = stmt.query_map([], |row| {
        Ok(FileRecord {
            id: row.get(0)?,
            filename: row.get(1)?,
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
    let sql = if let Some(group) = group_sort {
        println!("Grouping duplicate record search by {}", group);
        let null_text = if group_null {
            println!("Records without a {} entry will be processed together.", group);
            "".to_string()
        } else {
            println!("Records without a {} entry will be skipped.", group);
            format!("WHERE {} IS NOT NULL AND {} != ''", group, group)
        };
        format!(
            "
            WITH ranked AS (
                SELECT
                    rowid AS id,
                    filename,
                    ROW_NUMBER() OVER (
                        PARTITION BY {}, filename
                        ORDER BY {}
                    ) as rn
                FROM justinmetadata {}
            )
            SELECT id, filename FROM ranked WHERE rn > 1
            ",
            group, order_clause, null_text
        )
    } else {
        format!(
            "
            WITH ranked AS (
                SELECT
                    rowid AS id,
                    filename,
                    ROW_NUMBER() OVER (
                        PARTITION BY filename
                        ORDER BY {}
                    ) as rn
                FROM justinmetadata
            )
            SELECT id, filename FROM ranked WHERE rn > 1
            ",
            order_clause
        )
    };

    // Execute the query and gather the duplicates
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(FileRecord {
            id: row.get(0)?,
            filename: row.get(1)?,
        })
    })?;

    for file_record in rows {
        file_records.insert(file_record?);
    }

    println!("Marked {} duplicate records for deletion.", file_records.len());

    Ok(file_records)
}

fn gather_filenames_with_tags(conn: &mut Connection, verbose: bool) -> Result<HashSet<FileRecord>> {
    println!("Searching {} for filenames containing tags", get_connection_source_filepath(&conn));
    let mut file_records = HashSet::new();
    let mut processed_files = HashSet::new();
    let tags = get_tags("TAG_FILE_PATH")?;

    let mut tags_found: usize = 0;
    for tag in tags {
        let query = format!("SELECT rowid, filename FROM justinmetadata WHERE filename LIKE '%' || ? || '%'");
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map([tag.clone()], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                filename: row.get(1)?,
            })
        })?;

        for file_record in rows {
            let file_record = file_record?;
            if processed_files.insert(file_record.filename.clone()) {
                file_records.insert(file_record);
            }
        }

        if verbose && !processed_files.is_empty() {
            println!("Filenames found for tag '{}': {}", tag, processed_files.len()-tags_found);
            tags_found = processed_files.len();
        }
    }
    println!("{} total records containing tags marked for deletion", file_records.len());
    Ok(file_records)
}


// DELETE FUNCTION
fn delete_file_records(conn: &mut Connection, records: &HashSet<FileRecord>, verbose: bool) -> Result<()> {
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
                print!(".");
            }
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            let query = format!("DELETE FROM justinmetadata WHERE rowid IN ({})", placeholders);
            let params: Vec<&dyn rusqlite::types::ToSql> = chunk.iter().map(|record| &(record.id) as &dyn rusqlite::types::ToSql).collect();
            tx.execute(&query, params.as_slice()).map(|_| ())
    })?;

    println!("");
    tx.commit()?;

    Ok(())
}


fn main() -> Result<(), Box<dyn Error>> {
    println!("SMDupeRemover v{}", VERSION);

    let args: Vec<String> = env::args().collect();
    let config = Config::new(&args)?;

    let source_db_path = &config.target_db.unwrap();
    let work_db_path = format!("{}_thinned.sqlite", &source_db_path.trim_end_matches(".sqlite"));
    fs::copy(&source_db_path, &work_db_path)?;

    let mut conn = Connection::open(&work_db_path)?; 
    println!("{} Total Records fround in {}", get_db_size(&conn), source_db_path);
  
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
        let ids_from_tags = gather_filenames_with_tags(&mut Connection::open(&work_db_path).unwrap(), config.verbose)?;
        all_ids_to_delete.extend(ids_from_tags);
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
    println!("Proceeding with deletion.");  

    // Perform deletion
    delete_file_records(&mut Connection::open(&work_db_path).unwrap(), &all_ids_to_delete, config.verbose)?;
    vacuum_db(&conn)?;
    println!("Removed {} records.", all_ids_to_delete.len());

    if config.duplicate_db && all_ids_to_delete.len() > 0 {
        create_duplicates_db(&source_db_path, &all_ids_to_delete)?;
    }

    if config.safe {
        println!("Original database intact.\nThinned records database moved to: {}", work_db_path);
    } else {    
        fs::copy(&work_db_path, &source_db_path)?;
        std::fs::remove_file(work_db_path)?;
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
    -c, --compare <database>          Compare with another database
    -d, --create-duplicates-database  Generates an additional _dupes database of all files that were removed
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
    // println!("SMDupeRemover v{}\n", VERSION);
    println!("{}", help_message);
}

fn generate_config_files(tjf: bool) -> Result<()> {
    if tjf {
        create_tjf_order_file()?;
    } else {
        create_order_file()?;
    }
    
    let mut tags_file = File::create(TAG_FILE_PATH).unwrap();
    for tag in DEFAULT_TAGS {
        writeln!(tags_file, "{}", tag).unwrap();
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


const TJF_DEFAULT_ORDER: [&str; 12] = [
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