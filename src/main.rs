use rusqlite::{Connection, Result};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::error::Error;
use terminal_size::{Width, terminal_size};

const BATCH_SIZE: usize = 10000;

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

#[derive(Hash, Eq, PartialEq, Debug)]
struct FileRecord {
    id: usize,
    filename: String,
}

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

    tx.commit()?;

    Ok(())
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

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--generate-config-files" => generate_config_files().unwrap(),
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
            print_help();
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
 
 fn get_db_size(conn: &Connection,) -> usize {

     // Get the count of remaining rows
      let count: usize = conn.query_row(
          "SELECT COUNT(*) FROM justinmetadata",
          [],
          |row| row.get(0)
      ).unwrap();
      count
 }


//COMPARE DATABASES SECTION
fn fetch_filenames(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT filename FROM justinmetadata")?;
    let filenames: HashSet<String> = stmt.query_map([], |row| row.get(0))?
        .filter_map(Result::ok)
        .collect();
    Ok(filenames)
}

fn delete_filenames(conn: &mut Connection, filenames: &HashSet<String>) -> Result<()> {
    let tx = conn.transaction()?;

    // Convert the filenames into a vector and batch process
    filenames
        .iter()
        .collect::<Vec<_>>()
        .chunks(BATCH_SIZE)
        .try_for_each(|chunk| {
            let _ = io::stdout().flush();
            print!(".");
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            let query = format!("DELETE FROM justinmetadata WHERE filename IN ({})", placeholders);
            let params: Vec<&dyn rusqlite::types::ToSql> = chunk.iter().map(|s| *s as &dyn rusqlite::types::ToSql).collect();
            tx.execute(&query, params.as_slice()).map(|_| ())
        })?;

    tx.commit()?;

    Ok(())
}

fn compare_databases(target_db: &str, compare_db: &str, prompt: bool) -> Result<usize> {
    println!("Comparing filenames between {} and {}", target_db, compare_db);
    
    let conn_a = Connection::open(compare_db)?;
    let mut conn_b = Connection::open(target_db)?;

    // Fetch filenames from both databases
    let filenames_a = fetch_filenames(&conn_a)?;
    let filenames_b = fetch_filenames(&conn_b)?;

    // Calculate the common filenames
    let common_filenames: HashSet<_> = filenames_a.intersection(&filenames_b).cloned().collect();
    
    if common_filenames.is_empty() {
        println!("NO OVERLAPPING FILENAMES FOUND!");
        return Ok(0); // Early exit if no duplicates are found
    }

    // Inform the user about the overlap and prompt for deletion confirmation
    println!(
        "Found {} overlapping filenames between {} and {}. (does not include duplicates)",
        common_filenames.len(),
        target_db,
        compare_db
    );

    if prompt {
        println!("Type 'yes' to remove them: ");
        let mut user_input = String::new();
        io::stdin()
            .read_line(&mut user_input)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(e),
            ))?;
        
        if user_input.trim().eq_ignore_ascii_case("yes") {
            println!("Proceeding with deletion.");
        } else {
            println!("Aborted deletion.");
            return Ok(0); // Early exit if user aborts deletion
        }
    }

    // Get the initial size of the target database
    let initial_db_size = get_db_size(&conn_b);
    
    // Proceed with deletion of common filenames
    delete_filenames(&mut conn_b, &common_filenames)?;

    // Calculate the number of removed files
    let files_removed = initial_db_size - get_db_size(&conn_b);
    println!("Removed {} files from {}", files_removed, target_db);

    Ok(files_removed)
}
//END COMPARE DB SECTION

//SEARCH FOR DUPES SECTION
fn read_order(file_path: &str) -> Result<Vec<String>, io::Error> {
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

fn remove_duplicate_filenames_from_database(db_path: &str, prompt: bool, verbose: bool, group_sort: &Option<String>, skip_null: bool) -> Result<usize, rusqlite::Error> {
    println!("Searching for Duplicate Filenames in: {}", db_path);
    if let Some(group) = group_sort {println!("Grouping by {}", group)};

    let mut conn = Connection::open(db_path)?;
    let mut db_size = get_db_size(&conn);

    // Read the order file
    let order_file = "SMDupe_order.txt";
    let order = read_order(order_file).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

    // Construct the ORDER BY clause dynamically
    let order_clause = order.join(", ");

    // Start a transaction
    let mut tx = conn.transaction()?;

    let mut sql = String::new();
    if let Some(group) = group_sort {
        let null_text = if skip_null {format!("WHERE {} IS NOT NULL AND {} != ''", group, group)}
                            else {"".to_string()};
        sql = format!(
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
            ", group, order_clause, null_text
        );
    } else {
        sql = format!(
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
        );
    }

    // Find the best record for each filename based on the given criteria
    let mut ids_to_delete: Vec<(i64, String)> = {
        let mut stmt = tx.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.filter_map(Result::ok).collect()
    }; // stmt is dropped here
    tx.commit()?;
    ids_to_delete.sort_by(|a, b| b.0.cmp(&a.0));
    
    if ids_to_delete.len() == 0 {
        println!("ALL FILENAMES are UNIQUE! in {}", db_path);
        return Ok(0); // Exit the function early if no duplicates are found
    }
    
    //Get Terminal Size Width for the verbose mode
    let (width, _) = terminal_size().unwrap_or((Width(80), terminal_size::Height(0)));
    let line_width = width.0 as usize;


    
        
        println!("Found {} Duplicate Filenames.", ids_to_delete.len());
        if prompt {
            println!("Type 'yes' to remove them: ");
            let mut user_input = String::new();
            io::stdin().read_line(&mut user_input).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
            let user_input = user_input.trim().to_lowercase();

            if user_input != "yes" {
                println!("Aborted deletion.");
                return Ok(0);
            }
        }
        println!("Proceeding with Deletion");
        if verbose {
            tx = conn.transaction()?;
            for (id, filename) in ids_to_delete {
                print!("\r{}", " ".repeat(line_width)); print!("\rRemoving ID: {}, Filename: {}", id, filename); let _ = io::stdout().flush();
                tx.execute("DELETE FROM justinmetadata WHERE rowid = ?", [id])?;
            }
            tx.commit()?;
        } else {
            delete_rows_in_batches(&mut conn, ids_to_delete.into_iter().map(|(num, _)| num).collect())?;
        }



        db_size -= get_db_size(&conn);
        println!("\nRemoved {} Entries from {}", db_size, db_path);
    

    Ok(db_size)
}
//END SEARCH FOR DUPES

//SEARCH FOR TAGS SECTION


fn read_tags(file_path: &str) -> Result<Vec<String>, rusqlite::Error> {
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


fn remove_filesnames_with_tags_from_database(db_path: &str, prompt: bool, verbose: bool) -> Result<usize> {
    let mut conn = Connection::open(db_path)?;
    let tags = read_tags("SMDupe_tags.txt")?;

    // Count the number of rows that match the tags
    let total_rows_found = count_rows_with_tags(&mut conn, &tags, verbose)?;

    if total_rows_found == 0 {
        println!("No rows found with the specified tags.");
        return Ok(0);
    }

    println!("Found {} filenames with matching tags. Some Overlap is possible", total_rows_found);
    if prompt {
        // Prompt the user
        println!("Type 'yes' to remove them from {}.", db_path);
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        let user_input = user_input.trim().to_lowercase();

        if user_input != "yes" {
            println!("Aborted deletion.");
            return Ok(0);
            // If user confirms, remove the rows
        }
    }
    println!("Proceeding with deletion");
    let rows_deleted = delete_rows_with_tags(&mut conn, &tags, verbose)?;
    println!("Actually Deleted {} rows from {}", rows_deleted, db_path);

    Ok(rows_deleted)
}

fn delete_rows_with_tags(conn: &mut Connection, tags: &[String], verbose: bool) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut total_rows_deleted = 0;

    for tag in tags {
        let query = format!("DELETE FROM justinmetadata WHERE filename LIKE '%' || ? || '%'");
        let rows_deleted = tx.execute(&query, &[tag])?;
        if rows_deleted > 0 && verbose {
            println!("Filenames removed for tag '{}': {}", tag, rows_deleted);
        } else {
            let _ = io::stdout().flush();
            print!(".");
        }

        total_rows_deleted += rows_deleted;
    }
    println!{""};

    tx.commit()?;

    Ok(total_rows_deleted)
}

fn count_rows_with_tags(conn: &mut Connection, tags: &[String], verbose: bool) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut total_rows_found = 0;
    let mut processed_files = std::collections::HashSet::new();

    for tag in tags {
        let query = format!("SELECT filename FROM justinmetadata WHERE filename LIKE '%' || ? || '%'");
        let mut stmt = tx.prepare(&query)?;
        let rows = stmt.query_map(&[tag], |row| row.get::<_, String>(0))?;

        let mut count = 0;
        for filename in rows {
            let filename = filename?;
            if processed_files.insert(filename.clone()) {
                count += 1;
            }
        }
        
        if count > 0 && verbose {
            println!("Filenames found for tag '{}': {}", tag, count);
        }
        total_rows_found += count;
    }

    tx.commit()?;
    Ok(total_rows_found)
}
//END SEARCH FOR TAGS



// DUPLICATES DATABASE SECTION
fn delete_rows_in_batches(conn: &mut Connection, ids_to_remove: Vec<i64>) -> Result<()> {
    let tx = conn.transaction()?;

    // Split the ids into batches if necessary
    const BATCH_SIZE: usize = 10000;
    for chunk in ids_to_remove.chunks(BATCH_SIZE) {
        let _ = io::stdout().flush();
        print!(".");
        let placeholders: Vec<String> = chunk.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "DELETE FROM justinmetadata WHERE rowid IN ({})",
            placeholders.join(", ")
        );

        // Convert the chunk to a Vec<&dyn rusqlite::types::ToSql>
        let params: Vec<&dyn rusqlite::types::ToSql> = chunk.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();

        // Pass the parameters to `tx.execute`
        tx.execute(&query, params.as_slice())?;
    }

    tx.commit()?;

    Ok(())
}

fn create_duplicates_db(db_path: &str, processed_conn: &Connection, _verbose: bool) -> Result<()> {
    println!("Generating Duplicates Only Database (this can be slow if your database is huge)");
    let duplicate_db_path = format!("{}_dupes.sqlite", &db_path.trim_end_matches(".sqlite"));
    fs::copy(&db_path, &duplicate_db_path).unwrap();
    let mut dupe_conn = Connection::open(&duplicate_db_path)?;
    
    // Get IDs of rows in the processed database
    let ids_to_remove: Vec<i64> = {
        let mut stmt = processed_conn.prepare("SELECT rowid FROM justinmetadata")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.filter_map(Result::ok).collect()
    };

    // if verbose { delete_rows(&mut dupe_conn, ids_to_remove, false)?;
    // } else {
        delete_rows_in_batches(&mut dupe_conn, ids_to_remove)?;
    // }

    dupe_conn.execute("VACUUM", [])?; // Execute VACUUM on the dupe connection   
 
    // Get the count of remaining rows
    let remaining_count: usize = get_db_size(&dupe_conn);

    println!("\n{} records moved to {}", remaining_count, duplicate_db_path);

    Ok(())
}
//END DUPES SECTION



fn main() -> Result<(), Box<dyn Error>> {

    let args: Vec<String> = env::args().collect();
    let config = Config::new(&args)?;

    let source_db_path = &config.target_db.unwrap();
    let work_db_path = format!("{}_thinned.sqlite", &source_db_path.trim_end_matches(".sqlite"));
    fs::copy(&source_db_path, &work_db_path)?;
     
    let mut total = 0;

    if let Some(compare_db_path) = config.compare_db {
        total += compare_databases(&work_db_path, &compare_db_path, config.prompt)?;
    }

    if config.filename_check {
        total += remove_duplicate_filenames_from_database(&work_db_path, config.prompt, config.verbose, &config.group_sort, config.group_null )?;
    }

    if config.prune_tags {
        total += remove_filesnames_with_tags_from_database(&work_db_path, config.prompt, config.verbose)?;
    }

    if total > 0 {
        vacuum_db(&work_db_path)?;
        println!("{} Total Entries removed from Database", total);
    }

    if config.duplicate_db {
        create_duplicates_db(&source_db_path, &Connection::open(&work_db_path).unwrap(), config.verbose)?;
    }

    if !config.safe {
        fs::copy(&work_db_path, &source_db_path)?;
        std::fs::remove_file(work_db_path)?;
    }

    Ok(())
}

fn vacuum_db(db_path: &str) -> Result<()> { 
    println!("Cleaning up Database {}", db_path);
    let conn = Connection::open(&db_path)?;
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

fn generate_config_files() -> Result<()> {
    // Generate SMDupe_order.txt and SMDupe_tags.txt with default values
    let order_file_path = "SMDupe_order.txt";

    let mut order_file = File::create(order_file_path).unwrap();
    writeln!(order_file, "## Column in order of Priority and whether it should be DESCending or ASCending.  Hashtag will bypass").unwrap();
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
    return Ok(());
}

